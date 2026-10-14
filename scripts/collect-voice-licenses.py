"""Collect notices and corresponding copyleft sources for the packaged worker.
Run with the voice build environment's Python. Network is build-time only.
"""
import argparse
import hashlib
import importlib.metadata as metadata
import json
import shutil
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, default=root / "src-tauri/resources/licenses/voice")
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
packages = []
for distribution in metadata.distributions():
    name = distribution.metadata["Name"]
    if name.lower() == "av":
        continue  # Build environment only; encoded-media decoding is excluded.
    folder = out / "packages" / name
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "METADATA.txt").write_text(distribution.read_text("METADATA") or "", encoding="utf-8")
    for file in distribution.files or []:
        if any(word in str(file).lower() for word in ("license", "copying", "notice", "copyright")) and str(file).lower().endswith((".txt", ".md", ".rst", "license", "copying", "notice")):
            path = Path(distribution.locate_file(file))
            if path.is_file():
                destination = folder / Path(file).name
                if destination.exists():
                    destination = folder / (hashlib.sha256(str(file).encode()).hexdigest()[:8] + "-" + Path(file).name)
                shutil.copy2(path, destination)
    packages.append({"name":name,"version":distribution.version,"license":distribution.metadata.get("License-Expression") or distribution.metadata.get("License"),"homepage":distribution.metadata.get_all("Project-URL") or distribution.metadata.get("Home-page")})

def retrieve(url, filename, checksum=None):
    destination = out / "sources" / filename
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        urllib.request.urlretrieve(url, destination)
    digest = hashlib.file_digest(destination.open("rb"), "sha256").hexdigest()
    if checksum and digest != checksum:
        raise ValueError(f"Source checksum mismatch: {filename}")
    return {"url":url,"file":filename,"sha256":digest}

sources = [retrieve("https://files.pythonhosted.org/packages/02/cc/95b18b58d9c235d8e0bcece7321b7a7347873c14f9fea2c734b31ef04ff2/piper_tts-1.8.0.tar.gz", "piper_tts-1.8.0.tar.gz", "830588aded347df579c91a32703e0fc2a3685d84f1e3533b14f2de69135d4904")]
sources += [
    retrieve("https://codeload.github.com/OHF-Voice/piper1-gpl/tar.gz/639388b6317fc4731e91d53da42aea68fd4166ff", "piper-complete-639388b.tar.gz"),
    retrieve("https://codeload.github.com/espeak-ng/espeak-ng/tar.gz/724808c5a83f9ef95fdd0db886ba7ba537ff224a", "espeak-ng-724808c.tar.gz"),
]
for package, version in [("PyInstaller", "6.16.0")]:
    with urllib.request.urlopen(f"https://pypi.org/pypi/{package}/{version}/json") as response:
        release = json.load(response)
    entry = next(file for file in release["urls"] if file["packagetype"] == "sdist")
    sources.append(retrieve(entry["url"], entry["filename"], entry["digests"]["sha256"]))
for filename in ("transcribe.py", "nabra.py", "diacritize.py", "duplex.py", "requirements-lock.txt", "requirements-macos-intel.txt", "patch_faster_whisper.py"):
    shutil.copy2(root / "voice" / filename, out / filename)
shutil.copy2(root / "scripts/build-voice.ps1", out / "build-voice.ps1")
for filename in ("build-macos-intel.sh", "stage-macos-resources.py", "verify-macos-models.py", "extract-macos-llama.py"):
    shutil.copy2(root / "scripts" / filename, out / filename)
shutil.copy2(Path(metadata.distribution("faster-whisper").locate_file("faster_whisper/audio.py")), out / "faster-whisper-audio-modified.py")
(out / "PACKAGES.json").write_text(json.dumps(packages, indent=2), encoding="utf-8")
(out / "SOURCES.json").write_text(json.dumps(sources, indent=2), encoding="utf-8")
print(f"Collected {len(packages)} package notices and {len(sources)} source archives.")
