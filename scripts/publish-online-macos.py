"""Publish verified Intel DMG parts only to the owner-authorized repository."""
import hashlib
import http.client
import json
import os
import urllib.parse
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
repo = "mhddd171171-byte/Inventory"
assert os.environ["GITHUB_REPOSITORY"] == repo
report = json.loads((root / ".build/MACOS-ONLINE-PACKAGE.json").read_text())
assert report["passed"]
version = json.loads((root / "package.json").read_text())["version"]
dmg = next((root / "src-tauri/target/x86_64-apple-darwin/release/bundle/dmg").glob("*.dmg"))
headers = {"Authorization": "Bearer " + os.environ["GH_TOKEN"], "Accept": "application/vnd.github+json",
           "User-Agent": "Inventory-Release", "X-GitHub-Api-Version": "2022-11-28"}
def api(path, data, method="POST"):
    request = urllib.request.Request(f"https://api.github.com/repos/{repo}/{path}",
                data=json.dumps(data).encode(), headers={**headers, "Content-Type": "application/json"}, method=method)
    with urllib.request.urlopen(request, timeout=90) as response:
        return json.load(response)
tag = f"macos-v{version}-build{os.environ['GITHUB_RUN_ID']}-{os.environ['GITHUB_RUN_ATTEMPT']}"
release = api("releases", {"tag_name": tag, "target_commitish": os.environ["GITHUB_SHA"], "draft": True,
    "prerelease": True, "name": f"Inventory Manager {version} — Intel Mac online inventory",
    "body": "Shared Turso/libSQL inventory, equal full access without user profiles, validated .db import/restore, automatic local backups, revision-verified caching and linked warranty CRs with accurate FIFO cost/profit. Connection controls are in Settings; Warranty is inside each product line.\n\n"
    "Inventory requires an internet connection and a privately configured database key. AI and voice inference remain local, with the same verified offline models/runtimes as 2.0.11 included. No cloud AI API, Python installation, Terminal or model download is required on the customer's computer.\n\n"
    "Intel macOS 13+. Ad-hoc signed, not Apple-notarized; macOS may require Privacy & Security > Open Anyway. Native tests, mounted DMG/signature/resource checks and an installed launch/local-AI lifecycle smoke test run before publishing. Physical customer hardware/audio and an authenticated Turso operation on Mac have not been tested; live Turso business operations were verified through the Windows client. No database key or customer records are embedded.\n\n"
    "Download parts are for the distributor. The distributor assembles the single verified DMG and gives only that file to the customer. Stable 2.0.11 installers remain available."})
upload = urllib.parse.urlsplit(release["upload_url"].split("{", 1)[0])
assert upload.hostname == "uploads.github.com" and upload.path.startswith(f"/repos/{repo}/")
def send(name, size, source):
    digest = hashlib.sha256()
    connection = http.client.HTTPSConnection(upload.hostname, timeout=600)
    try:
        connection.putrequest("POST", upload.path + "?name=" + urllib.parse.quote(name))
        for key, value in {**headers, "Content-Type": "application/octet-stream", "Content-Length": str(size)}.items():
            connection.putheader(key, value)
        connection.endheaders()
        remaining = size
        while remaining:
            block = source.read(min(4 * 1024 * 1024, remaining))
            assert block, "Unexpected end of installer"
            digest.update(block)
            connection.send(block)
            remaining -= len(block)
        response = connection.getresponse()
        data = response.read()
        if response.status != 201:
            raise RuntimeError(f"Asset upload failed with HTTP {response.status}")
    finally:
        connection.close()
    return digest.hexdigest()
with dmg.open("rb") as source:
    whole = hashlib.file_digest(source, "sha256").hexdigest()
manifest = {"filename": dmg.name, "bytes": dmg.stat().st_size, "sha256": whole,
            "source_commit": os.environ["GITHUB_SHA"], "parts": []}
with dmg.open("rb") as source:
    remaining = dmg.stat().st_size
    number = 1
    while remaining:
        size = min(1536 * 1024 * 1024, remaining)
        name = f"{dmg.name}.part{number:02d}"
        digest = send(name, size, source)
        manifest["parts"].append({"name": name, "bytes": size, "sha256": digest})
        remaining -= size
        number += 1
        print("Uploaded verified part", number - 1, flush=True)
path = root / ".build/INSTALLER-PARTS.json"
path.write_text(json.dumps(manifest, indent=2))
for path in [path, root / ".build/MACOS-ONLINE-PACKAGE.json", root / ".build/MACOS-ONLINE-APP.json", root / ".build/MACOS-SHA256SUMS.txt"]:
    with path.open("rb") as source:
        send(path.name, path.stat().st_size, source)
api(f"releases/{release['id']}", {"draft": False}, method="PATCH")
print(release["html_url"])
