"""Aggregate only measured receipts; never launch anything or read private snapshots."""
import csv
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "reports/phase1-4-validation"
rows = []


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def add(variant, operation, sample, value, unit, method):
    if value is not None:
        rows.append([variant, operation, sample, value, unit, method])


baseline = read(DATA / "baseline/PROBES.json")
for sample, run in enumerate(baseline, 1):
    for key, unit in [("wall_ms", "ms"), ("cpu_ms", "ms"), ("sampled_peak_bytes", "bytes")]:
        add("phase1.3", run["mode"] + "_" + key, sample, run[key], unit, "headless process; 20ms parent sampling; observed OS cache state")
    for name, count, milliseconds in re.findall(r"([^\n:]+): (\d+) entries, ([\d.]+) ms", run["stdout"]):
        add("phase1.3", name.strip() + "_discovery", sample, milliseconds, "ms", "unchanged discovery provider's internal timer")
        add("phase1.3", name.strip() + "_entries", sample, count, "entries", "raw before Index deduplication")
    match = re.search(r"Discovery total ([\d.]+) ms", run["stdout"])
    if match:
        add("phase1.3", "full_discovery", sample, match[1], "ms", "unchanged discovery; no Index or GUI initialization")
add("phase1.3", "executable_size", 1, (ROOT / "target/release/spotlight-launcher-phase1-3.exe").stat().st_size, "bytes", "preserved executable stat")

for variant in ["baseline", "default", "strip", "codegen1", "final"]:
    folder = DATA / variant
    profile = folder / "PROFILE.json"
    if profile.exists():
        document = read(profile)
        add(variant, "executable_size", 1, document["executable_bytes"], "bytes", "Windows PE file size")
        for sample, run in enumerate(document["runs"], 1):
            for key, unit in [("wall_ms", "ms"), ("cpu_ms", "ms"), ("sampled_peak_bytes", "bytes")]:
                add(variant, run["mode"] + "_" + key, sample, run[key], unit, document["method"])
    path = folder / "SEARCH_BENCHMARKS.csv"
    if path.exists():
        with path.open(newline="") as file:
            for row in csv.DictReader(file):
                for field in ["median_ms", "p95_ms"]:
                    add(variant, f"synthetic_{row['entries']}_{row['category']}_{field}", row["repetitions"], row[field], "ms", "unchanged fixture; 3 warmups then 25 repetitions")

with (DATA / "CACHE_FEASIBILITY.csv").open(newline="") as file:
    for row in csv.DictReader(file):
        add("cache_feasibility", row["operation"], row["iteration"], row["elapsed_ms"], "ms", f"private representative snapshot; {row['items']} raw entries; {row['bytes']} bytes; OS-cached; no GUI")

for name in ["CACHE_MISS", "CACHE_HIT", "CACHE_HIT_REPEAT", "CACHE_FINAL_HIT"]:
    document = read(DATA / (name + ".json"))
    for key in ["config_ms", "cache_load_ms", "cached_index_build_ms", "cached_usable_ms", "discovery_ms", "fresh_index_build_ms", "fresh_usable_ms", "cache_write_ms"]:
        add(name, key, 1, document[key], "ms", document["method"])
    for key, unit in [("cache_bytes", "bytes"), ("cached_entries", "entries"), ("fresh_entries", "entries")]:
        add(name, key, 1, document[key], unit, "cache eligible subset excludes parameterized shortcuts/unavailable/Modrinth profiles")
    for stage in ["after_cache", "with_both_indexes", "after_refresh"]:
        for key, value in document[stage].items():
            add(name, stage + "_" + key, 1, value, "ms" if key == "cpu_ms" else "bytes", "in-process Windows resource counters; backend only, not GUI/GPU memory")
    for provider in document["providers"]:
        add(name, provider["provider"] + "_discovery", 1, provider["milliseconds"], "ms", "unchanged discovery provider timer")
    for case in document["searches"]:
        for field in ["median_ms", "p95_ms"]:
            add(name, case["case"] + "_" + field, 25, case[field], "ms", "fixed local benchmark queries, not user query history; 3 warmups")

for name in ["DEFAULT", "STRIP", "CODEGEN", "FINAL"]:
    document = read(DATA / (name + "_BUILD.json"))
    add(name, "incremental_build", 1, document["seconds"], "seconds", "2 build jobs; existing dependency artifacts reused; not clean build time")

with (ROOT / "PERFORMANCE_BENCHMARKS.csv").open("w", newline="", encoding="utf-8") as file:
    writer = csv.writer(file)
    writer.writerow(["variant", "operation", "sample_or_repetitions", "value", "unit", "method"])
    writer.writerows(rows)
print(f"Recorded {len(rows)} measured rows")
