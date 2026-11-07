"""Generate derived icons only; never change the user-provided PNG."""
import hashlib
import json
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
source = root / "Ferrum.png"
digest = hashlib.sha256(source.read_bytes()).hexdigest()
image = Image.open(source).convert("RGBA")
if image.width != image.height:
    raise SystemExit("Expected square artwork; obtain owner direction instead of cropping")
sizes = [16, 24, 32, 48, 64, 128, 256]
image.save(root / "resources/ferrum.ico", format="ICO", sizes=[(n, n) for n in sizes], bitmap_format="png")
(root / "resources/ferrum-icon-32.rgba").write_bytes(image.resize((32, 32), Image.Resampling.LANCZOS).tobytes())
assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
(root / "resources/icon-source.json").write_text(json.dumps({
    "source": "Ferrum.png", "source_sha256": digest,
    "source_width": image.width, "source_height": image.height,
    "source_alpha_range": image.getchannel("A").getextrema(), "derived_sizes": sizes,
    "resampling": "Pillow 10.4.0 LANCZOS; aspect ratio retained; original not modified"
}, indent=2))
