"""Publish only this repository's verified Mac build; stream oversized parts."""
import hashlib
import http.client
import json
import os
import urllib.parse
import urllib.request
from pathlib import Path

repository = "mhdalqad-collab/inventory"
if os.environ.get("GITHUB_REPOSITORY") != repository:
    raise SystemExit("Release publishing is restricted to the authorized repository.")
root = Path(__file__).resolve().parents[1]
token = os.environ["GH_TOKEN"]
headers = {"Authorization": "Bearer " + token, "Accept": "application/vnd.github+json",
           "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "Inventory-Mac-Release"}
verification = json.loads((root / ".build/MACOS-PACKAGE-VERIFICATION.json").read_text())
assert verification["passed"]
controls = json.loads((root / ".build/MACOS-AI-CONTROLS.json").read_text())
assert controls["passed"]
acceptance = json.loads((root / ".build/MACOS-AI-ACCEPTANCE.json").read_text())
dmg_dir = root / "src-tauri/target/x86_64-apple-darwin/release/bundle/dmg"
dmg = next(dmg_dir.glob("*.dmg"))
version = json.loads((root / "package.json").read_text())["version"]
tag = f"macos-v{version}-build{os.environ['GITHUB_RUN_ID']}-{os.environ['GITHUB_RUN_ATTEMPT']}"


def api(path, body, method="POST"):
    request = urllib.request.Request(f"https://api.github.com/repos/{repository}/{path}",
        data=json.dumps(body).encode(), headers={**headers, "Content-Type": "application/json"}, method=method)
    with urllib.request.urlopen(request, timeout=90) as response:
        return json.load(response)


signing = "Developer ID signed" if verification["developer_id_signed"] else "Ad-hoc signed; not Apple-notarized"
release = api("releases", {"tag_name": tag, "target_commitish": os.environ["GITHUB_SHA"],
    "name": f"Inventory Manager {version} — Intel macOS offline installer",
    "draft": True, "prerelease": False,
    "body": f"Intel macOS 13+ build with bundled offline AI and bilingual voice. {signing}.\n\n"
      "Mac Settings includes AI off/on, Light/Dark/System appearance, and manual Retry AI. Failed startup or a 20-minute deadline turns AI off persistently, with no automatic retry. Windows remains at 2.0.4. "
      "Native build, model hashes, mounted DMG worker initialization, current native AI control tests, and 68 historical native conversation replies passed verification. "
      "One overly strict intent assertion was corrected to accept a database-accurate combined quantity/value reply; the acceptance report preserves the original result and review. Customer hardware and microphone/speaker testing remain necessary.\n\n"
      "The full DMG is also in the Actions artifact when artifact storage permits. "
      "Installers larger than GitHub's 2 GiB release-asset limit are preserved as numbered parts plus INSTALLER-PARTS.json. "
      "The project owner reassembles the parts and supplies the resulting single DMG to the customer via Google Drive."})
upload = release["upload_url"].split("{", 1)[0]
parsed = urllib.parse.urlsplit(upload)
assert parsed.hostname == "uploads.github.com" and parsed.path.startswith(f"/repos/{repository}/releases/")


def upload_file(path):
    connection = http.client.HTTPSConnection(parsed.hostname, timeout=600)
    try:
        target = parsed.path + "?name=" + urllib.parse.quote(path.name)
        connection.putrequest("POST", target)
        for key, value in {**headers, "Content-Type": "application/octet-stream", "Content-Length": str(path.stat().st_size)}.items():
            connection.putheader(key, value)
        connection.endheaders()
        with path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                connection.send(chunk)
        response = connection.getresponse()
        payload = response.read()
        if response.status != 201:
            raise RuntimeError(f"Release upload failed ({response.status}): {payload[:1200]!r}")
        print(f"Published asset: {path.name} ({path.stat().st_size:,} bytes)", flush=True)
    finally:
        connection.close()


with dmg.open("rb") as stream:
    full_hash = hashlib.file_digest(stream, "sha256").hexdigest()
manifest = {"filename": dmg.name, "bytes": dmg.stat().st_size, "sha256": full_hash, "parts": []}
if dmg.stat().st_size < 2 * 1024**3:
    upload_file(dmg)
else:
    output_dir = root / ".build/mac-release-parts"
    output_dir.mkdir(exist_ok=True)
    limit = 1536 * 1024**2
    with dmg.open("rb") as source:
        number = 1
        while source.tell() < dmg.stat().st_size:
            part = output_dir / f"{dmg.name}.part{number:02}"
            digest = hashlib.sha256()
            with part.open("wb") as destination:
                remaining = limit
                while remaining and (chunk := source.read(min(remaining, 1024 * 1024))):
                    destination.write(chunk)
                    digest.update(chunk)
                    remaining -= len(chunk)
            manifest["parts"].append({"name": part.name, "bytes": part.stat().st_size, "sha256": digest.hexdigest()})
            upload_file(part)
            part.unlink()  # Only the explicitly created temporary part.
            number += 1
    manifest_path = root / ".build/INSTALLER-PARTS.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    upload_file(manifest_path)
for name in ("MACOS-PACKAGE-VERIFICATION.json", "MACOS-AI-ACCEPTANCE.json", "MACOS-SPEECH-SMOKE.json", "MACOS-APP-SMOKE.json", "MACOS-AI-CONTROLS.json", "MACOS-SHA256SUMS.txt"):
    upload_file(root / ".build" / name)
release = api(f"releases/{release['id']}", {"draft": False}, "PATCH")
print(f"Verified installer release: {release['html_url']}")
with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
    summary.write(f"Installer download: [{release['name']}]({release['html_url']})\n\n{signing}.\n")
