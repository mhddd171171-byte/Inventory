"""Extract the checksum-verified source carried by the Mac build kit."""
import hashlib
import json
import tarfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
archive = root / "build-inputs/llama-b11554.tar.gz"
record = json.loads(archive.with_suffix(".json").read_text(encoding="utf-8"))
with archive.open("rb") as stream:
    actual = hashlib.file_digest(stream, "sha256").hexdigest()
if actual != record["sha256"]:
    raise SystemExit("llama.cpp source archive checksum mismatch.")
destination = root / ".build/llama-source"
destination.mkdir(parents=True, exist_ok=True)
with tarfile.open(archive, "r:gz") as source:
    source.extractall(destination, filter="data")
if not (destination / "llama.cpp-b11554/CMakeLists.txt").is_file():
    raise SystemExit("Unexpected source archive layout.")
print(destination / "llama.cpp-b11554")
