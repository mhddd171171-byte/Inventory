"""Reuse byte-verified AI/voice from our own tested 2.0.11 Intel DMG."""
import hashlib
import json
import os
import platform
import shutil
import subprocess
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = "mhddd171171-byte/Inventory"
TAG = "macos-v2.0.11-build40596634606-1"
BASE_SHA = "5868246a68115ff50c5a7a8324886072a72bd00abcee48001c7634960a1bf043"
PARTS = [
    ("part01", 1610612736, "9f5470744bd3b2e79ac62b54335f7d3c3d6761c7def7e3bc7d665cbff916dd63"),
    ("part02", 1610612736, "e40bc91dd34701829031d0d1473cf10e392b37e0a8fcc001c1badc15ed081cae"),
    ("part03", 339039096, "74670a09ce7f043c2474c70a675993be029431c7872f62df0ef2fbb76a8c9202"),
]
assert platform.system() == "Darwin" and platform.machine() == "x86_64"
assert os.environ.get("GITHUB_REPOSITORY") == REPO
cache = ROOT / ".build/baseline-download"
cache.mkdir(parents=True, exist_ok=True)

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

def github(path, binary=False):
    assert path.startswith(f"/repos/{REPO}/")
    headers = {"Accept": "application/octet-stream" if binary else "application/vnd.github+json",
               "Authorization": "Bearer " + os.environ["GH_TOKEN"], "User-Agent": "Inventory-Release"}
    request = urllib.request.Request("https://api.github.com" + path, headers=headers)
    try:
        return urllib.request.build_opener(NoRedirect()).open(request, timeout=120)
    except urllib.error.HTTPError as error:
        if error.code != 302:
            raise
        location = error.headers["Location"]
        parsed = urllib.parse.urlsplit(location)
        assert parsed.scheme == "https" and parsed.hostname.endswith(".githubusercontent.com")
        # Never forward the repository token to the signed asset download URL.
        return urllib.request.urlopen(location, timeout=120)

with github(f"/repos/{REPO}/releases/tags/{TAG}") as response:
    release = json.load(response)
dmg = cache / "baseline.dmg"
assert not dmg.exists()
with dmg.open("xb") as output:
    for suffix, size, expected in PARTS:
        asset = next(a for a in release["assets"] if a["name"].endswith(".dmg." + suffix))
        assert asset["size"] == size
        digest = hashlib.sha256()
        count = 0
        with github(f"/repos/{REPO}/releases/assets/{asset['id']}", binary=True) as incoming:
            while data := incoming.read(4 * 1024 * 1024):
                digest.update(data)
                count += len(data)
                output.write(data)
        assert count == size and digest.hexdigest() == expected
        print("Verified baseline", suffix, flush=True)
with dmg.open("rb") as stream:
    assert hashlib.file_digest(stream, "sha256").hexdigest() == BASE_SHA
mount = ROOT / ".build/baseline-mount"
mount.mkdir(exist_ok=True)
subprocess.run(["hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", str(mount), str(dmg)], check=True)
try:
    apps = list(mount.glob("*.app"))
    assert len(apps) == 1
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(apps[0])], check=True)
    source = apps[0] / "Contents/Resources/resources"
    stage = ROOT / ".build/macos-resources"
    assert not stage.exists(), "Preserve an existing resource stage rather than mixing builds"
    shutil.copytree(source, stage, symlinks=True)
    assert not list(stage.rglob("*.exe")) and not list(stage.rglob("*.dll"))
finally:
    subprocess.run(["hdiutil", "detach", str(mount)], check=True)
# The baseline is no longer needed; free the 3.5 GB before building the new DMG.
dmg.unlink()
source = ROOT / "src-tauri/resources"
models = [(Path("ai/Qwen3-4B-Instruct-2507-Q4_K_M.gguf"), "2fde00ce69dd4899c70d020845e2638353015bba0fdf161b3eb965f2bca4464e")]
for record in json.loads((source / "voice/MODELS.json").read_text()):
    models.append((Path("voice") / record["path"], record["sha256"]))
for relative, expected in models:
    path = stage / relative
    with path.open("rb") as stream:
        assert hashlib.file_digest(stream, "sha256").hexdigest() == expected
    destination = source / relative
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.hardlink_to(path)
for path in [stage / "ai/llama-server", stage / "voice/runtime/voice-worker"]:
    assert subprocess.check_output(["lipo", "-archs", str(path)], text=True).strip() == "x86_64"
print("Verified and reused offline Intel AI/voice resources; no model changes.", flush=True)
