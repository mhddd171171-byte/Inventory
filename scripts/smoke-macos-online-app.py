"""Installed Intel app/AI lifecycle; preserve an existing local DB untouched.

No Turso credential is supplied to CI. Cloud operations are separately tested
against the real test database on Windows; this report states that distinction.
"""
import hashlib
import json
import os
import plistlib
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
source = Path(sys.argv[1]).resolve()
app = Path.home() / "Applications" / source.name
assert not app.exists()
app.parent.mkdir(exist_ok=True)
subprocess.run(["ditto", "--rsrc", "--extattr", str(source), str(app)], check=True)
info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
directory = Path.home() / "Library/Application Support" / info["CFBundleIdentifier"] / "data"
directory.mkdir(parents=True, exist_ok=True)
fixture = root / ".build/MACOS-UPDATE-FIXTURE.db"
assert fixture.is_file() and not (directory / "inventory.db").exists()
shutil.copyfile(fixture, directory / "inventory.db")
original = hashlib.sha256((directory / "inventory.db").read_bytes()).hexdigest()
executable = app / "Contents/MacOS" / info["CFBundleExecutable"]
report = {"version": info["CFBundleShortVersionString"], "installed_launch": False,
          "automatic_ai_ready": False, "clean_exit": False, "owned_helpers_exited": False,
          "old_local_database_preserved": False, "live_turso_on_mac_tested": False,
          "physical_customer_mac_tested": False, "microphone_speaker_quality_tested": False}
owned = set()
stdout = (root / ".build/mac-online-stdout.log").open("w")
stderr = (root / ".build/mac-online-stderr.log").open("w")
process = subprocess.Popen([str(executable)], stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
try:
    report["installed_launch"] = True
    start = time.monotonic()
    while time.monotonic() - start < 180:
        assert process.poll() is None, "Installed app exited during startup"
        children = subprocess.run(["pgrep", "-P", str(process.pid)], capture_output=True, text=True)
        for value in children.stdout.split():
            pid = int(value)
            command = subprocess.check_output(["ps", "-ww", "-p", str(pid), "-o", "command="], text=True)
            if "llama-server" not in command and "voice-worker" not in command:
                continue
            owned.add(pid)
            port = re.search(r"--port\s+(\d+)", command)
            key = re.search(r"--api-key\s+(\S+)", command)
            if not port or not key:
                continue
            request = urllib.request.Request(f"http://127.0.0.1:{port[1]}/health",
                                             headers={"Authorization": "Bearer " + key[1]})
            try:
                with opener.open(request, timeout=2) as response:
                    report["automatic_ai_ready"] = response.status == 200
            except OSError:
                pass
        if report["automatic_ai_ready"]:
            break
        time.sleep(1)
    assert report["automatic_ai_ready"], "Bundled local model did not start automatically"
    report["ai_startup_seconds"] = round(time.monotonic() - start, 2)
    subprocess.run(["osascript", "-e", f"tell application {json.dumps(str(app))} to quit"],
                   check=True, timeout=20, capture_output=True)
    report["clean_exit"] = process.wait(timeout=30) == 0
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        living = []
        for pid in owned:
            try:
                os.kill(pid, 0)
                living.append(pid)
            except ProcessLookupError:
                pass
        if not living:
            report["owned_helpers_exited"] = True
            break
        time.sleep(.2)
    assert report["clean_exit"] and report["owned_helpers_exited"]
    report["old_local_database_preserved"] = hashlib.sha256((directory / "inventory.db").read_bytes()).hexdigest() == original
    assert report["old_local_database_preserved"]
    report["passed"] = True
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
    stdout.close()
    stderr.close()
    (root / ".build/MACOS-ONLINE-APP.json").write_text(json.dumps(report, indent=2))
print(json.dumps(report))
