"""Explicit build-time provisioning. Never called by the installed application."""
import hashlib
import json
import urllib.request
import zipfile
import shutil
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
ROOT = REPO_ROOT / "src-tauri" / "resources" / "voice"
PINS = REPO_ROOT / "voice/model-files.json"

def download(job):
    path = ROOT / job["path"]
    if not path.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("Invalid model resource path.")
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists() or hashlib.file_digest(path.open("rb"), "sha256").hexdigest() != job["sha256"]:
        temporary = path.with_suffix(path.suffix + ".partial")
        if "source_file" in job:
            source = REPO_ROOT / job["source_file"]
            shutil.copyfile(source, temporary)
        else:
            with urllib.request.urlopen(job["url"], timeout=90) as response, temporary.open("wb") as target:
                while chunk := response.read(1024 * 1024):
                    target.write(chunk)
            if "archive_member" in job:
                if hashlib.file_digest(temporary.open("rb"), "sha256").hexdigest() != job["source_sha256"]:
                    raise ValueError("Voice-pack source checksum mismatch.")
                with zipfile.ZipFile(temporary) as archive:
                    data = archive.read(job["archive_member"])
                temporary.write_bytes(data)
        if hashlib.file_digest(temporary.open("rb"), "sha256").hexdigest() != job["sha256"]:
            raise ValueError(f"Speech artifact checksum mismatch: {job['path']}")
        temporary.replace(path)
    digest = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
    print(f"Ready {path.name}: {path.stat().st_size:,} bytes", flush=True)
    if path.stat().st_size != job["bytes"]:
        raise ValueError("Speech artifact size mismatch.")
    return job

if __name__ == "__main__":
    jobs = json.loads(PINS.read_text(encoding="utf-8"))
    with ThreadPoolExecutor(max_workers=3) as pool:
        manifest = list(pool.map(download, jobs))
    (ROOT / "MODELS.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
