"""Read PE metadata only. Never execute a binary or load its DLLs."""
import argparse
import json
import os
import struct
from pathlib import Path


def inspect(path):
    data = Path(path).read_bytes()
    u16 = lambda offset: struct.unpack_from("<H", data, offset)[0]
    u32 = lambda offset: struct.unpack_from("<I", data, offset)[0]
    pe = u32(0x3C)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Not a Windows PE")
    optional = pe + 24
    directory = optional + (112 if u16(optional) == 0x20B else 96)
    sections = []
    start = optional + u16(pe + 20)
    for index in range(u16(pe + 6)):
        row = start + index * 40
        sections.append((u32(row + 12), max(u32(row + 8), u32(row + 16)), u32(row + 20)))

    def offset(rva):
        for address, size, raw in sections:
            if address <= rva < address + size:
                result = raw + rva - address
                if result >= len(data):
                    raise ValueError("PE address outside file")
                return result
        raise ValueError("Unmapped PE address")

    imports = []
    import_rva = u32(directory + 8)
    if import_rva:
        current = offset(import_rva)
        while any(data[current:current + 20]):
            name = offset(u32(current + 12))
            imports.append(data[name:data.index(b"\0", name)].decode("ascii"))
            current += 20
    resources = []
    resource_rva = u32(directory + 16)
    if resource_rva:
        base = offset(resource_rva)

        def visit(relative, ids=()):
            count = u16(base + relative + 12) + u16(base + relative + 14)
            for index in range(count):
                entry = base + relative + 16 + index * 8
                name, target = u32(entry), u32(entry + 4)
                if name & 0x80000000:
                    label = "named"
                else:
                    label = name
                if target & 0x80000000:
                    visit(target & 0x7FFFFFFF, ids + (label,))
                else:
                    position = base + target
                    address, size = u32(position), u32(position + 4)
                    resources.append((ids + (label,), data[offset(address):offset(address) + size]))
        visit(0)
    sizes = []
    for ids, payload in resources:
        if ids[0] == 14:
            count = struct.unpack_from("<H", payload, 4)[0]
            sizes += [payload[6 + index * 14] or 256 for index in range(count)]
    private_hits = []
    for name, prefix in [("developer_home", os.environ.get("USERPROFILE")), ("workspace", str(Path(__file__).resolve().parents[1]))]:
        if not prefix:
            continue
        for value in {prefix, prefix.replace("\\", "/")}:
            if any(value.encode(encoding).lower() in data.lower() for encoding in ("utf-8", "utf-16le")):
                private_hits.append(name)
                break
    return {"bytes": len(data), "machine": u16(pe + 4), "subsystem": u16(optional + 68),
            "imports": sorted(imports), "icon_sizes": sorted(set(sizes)),
            "version_resource_present": any(ids[0] == 16 for ids, _ in resources),
            "private_build_path_hits": private_hits,
            "vc_runtime_imports": [name for name in imports if name.lower().startswith(("vcruntime", "msvcp", "msvcr"))]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = inspect(args.binary)
    rendered = json.dumps(result, indent=2)
    if args.output:
        args.output.write_text(rendered)
    print(rendered)
    if result["machine"] != 0x8664 or result["subsystem"] != 2 or result["vc_runtime_imports"] or result["private_build_path_hits"] or result["icon_sizes"] != [16,24,32,48,64,128,256]:
        raise SystemExit("Release binary failed architecture/runtime/privacy/icon policy")
