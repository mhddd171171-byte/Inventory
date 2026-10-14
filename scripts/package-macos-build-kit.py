"""Transfer source and verified models to a Mac. This does NOT create a DMG."""
import hashlib
import json
import runpy
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
version = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["version"]
output_dir = ROOT / "deliverables/macOS-build-kit"
output_dir.mkdir(parents=True, exist_ok=True)
output = output_dir / f"InventoryManager-{version}-IntelMac-BUILD-KIT.zip"
source_zip = ROOT / f"deliverables/InventoryManager-{version}-source.zip"
models = runpy.run_path(str(ROOT / "scripts/verify-macos-models.py"))["verified_models"]()
model_paths = {model["path"] for model in models}
inputs = ROOT / ".build/macos-build-inputs"
inputs.mkdir(parents=True, exist_ok=True)
llama = inputs / "llama-b11554.tar.gz"
url = "https://codeload.github.com/ggml-org/llama.cpp/tar.gz/refs/tags/b11554"
if not llama.exists():
    print("Retrieving reviewed llama.cpp b11554 source (build time only).", flush=True)
    temporary = llama.with_suffix(".partial")
    urllib.request.urlretrieve(url, temporary)
    temporary.replace(llama)
with llama.open("rb") as stream:
    llama_digest = hashlib.file_digest(stream, "sha256").hexdigest()
record = {"version": "b11554", "url": url, "sha256": llama_digest, "bytes": llama.stat().st_size}
provenance = llama.with_suffix(".json")
if provenance.exists() and json.loads(provenance.read_text())["sha256"] != llama_digest:
    raise ValueError("Cached llama.cpp source changed.")
provenance.write_text(json.dumps(record, indent=2), encoding="utf-8")
manifest = {"application_version": version, "artifact_type": "developer build kit, NOT an installer",
            "native_mac_build_performed": False, "models": models, "llama_source": record,
            "target": "x86_64-apple-darwin", "candidate_minimum_macos": "13.0"}
written = set()
with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=1,
                     strict_timestamps=False, allowZip64=True) as archive:
    with zipfile.ZipFile(source_zip) as source:
        for info in source.infolist():
            name = info.filename
            if name in model_paths:
                continue
            path = ROOT / name
            if name == "SOURCE-PACKAGE.md":
                continue
            if Path(name).suffix.lower() in {".db", ".exe", ".dll", ".p12", ".key"}:
                raise ValueError(f"Unexpected private or Windows binary input: {name}")
            if path.is_file():
                archive.write(path, name)
            else:
                archive.writestr(name, source.read(name))
            written.add(name)
    for folder in ("scripts", "docs"):
        for path in sorted((ROOT / folder).glob("*")):
            name = path.relative_to(ROOT).as_posix()
            if path.is_file() and name not in written and path.suffix in {".py", ".sh", ".ps1", ".md"}:
                archive.write(path, name)
                written.add(name)
    for model in models:
        print(f"Including verified offline model: {model['path']}", flush=True)
        archive.write(ROOT / model["path"], model["path"], compress_type=zipfile.ZIP_STORED)
    archive.write(llama, "build-inputs/llama-b11554.tar.gz", compress_type=zipfile.ZIP_STORED)
    archive.writestr("build-inputs/llama-b11554.tar.json", json.dumps(record, indent=2))
    archive.writestr("MACOS-BUILD-MANIFEST.json", json.dumps(manifest, indent=2))
    archive.writestr("READ-ME-FIRST.md", (ROOT / "docs/MACOS-BUILD-KIT.md").read_text(encoding="utf-8"))
print("Checking complete ZIP integrity.", flush=True)
with zipfile.ZipFile(output) as archive:
    failure = archive.testzip()
    if len(archive.namelist()) != len(set(archive.namelist())):
        raise ValueError("Build kit contains duplicate paths.")
    if failure:
        raise ValueError(f"Build kit ZIP CRC failed: {failure}")
with output.open("rb") as stream:
    digest = hashlib.file_digest(stream, "sha256").hexdigest()
(output_dir / "SHA256SUMS.txt").write_text(f"{digest}  {output.name}\n", encoding="utf-8")
(output_dir / "BUILD-KIT-VERIFICATION.json").write_text(json.dumps({**manifest, "zip_crc_passed": True,
        "archive": output.name, "archive_bytes": output.stat().st_size, "archive_sha256": digest}, indent=2), encoding="utf-8")
print(f"Verified developer build kit: {output.name} ({output.stat().st_size:,} bytes).", flush=True)
