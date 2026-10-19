"""Launch the mounted app and check its automatic, owned AI lifecycle on CI."""
import json
import os
import plistlib
import re
import sqlite3
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
source_app = Path(sys.argv[1]).resolve()
app=Path.home()/"Applications"/source_app.name
assert not app.exists(), "CI installation target unexpectedly exists; preserve it."
app.parent.mkdir(exist_ok=True)
subprocess.run(["ditto","--rsrc","--extattr",str(source_app),str(app)],check=True)
info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
executable = app / "Contents/MacOS" / info["CFBundleExecutable"]
data_dir=Path.home()/"Library/Application Support"/info["CFBundleIdentifier"]/"data"
data_dir.mkdir(parents=True,exist_ok=True)
fixture=root/".build/MACOS-UPDATE-FIXTURE.db"
assert fixture.is_file() and not (data_dir/"inventory.db").exists()
# Seed the installed app with Windows-style schema SQL, preserving all fixture
# business rows. The original 2.0.6 exact-text validator rejects this valid file.
with sqlite3.connect(f"file:{fixture}?mode=ro",uri=True) as incoming, sqlite3.connect(data_dir/"inventory.db") as destination:
    schema=(root/"src-tauri/migrations/002_single_user.sql").read_text().replace("\r\n","\n").replace("\n","\r\n")
    destination.executescript(schema)
    for table in ("products","stock_entries","sales","sale_lines","fifo_allocations","legacy_archive","migration_info"):
        rows=incoming.execute(f"SELECT * FROM {table} ORDER BY rowid").fetchall()
        if rows: destination.executemany(f"INSERT INTO {table} VALUES ("+",".join("?" for _ in rows[0])+")",rows)
    assert destination.execute("PRAGMA integrity_check").fetchone()[0]=="ok"

def business_records(path):
    with sqlite3.connect(f"file:{path}?mode=ro",uri=True) as db:
        return {table:db.execute(f"SELECT * FROM {table} ORDER BY rowid").fetchall() for table in ("products","stock_entries","sales","sale_lines","fifo_allocations")}
expected=business_records(fixture)
report = {"application_launched_from_dmg": False,"application_launched_from_applications":False, "automatic_ai_ready": False,
          "clean_application_exit": False, "owned_ai_exited": False}
owned = set()
stdout = (root / ".build/mac-app-stdout.log").open("w")
stderr = (root / ".build/mac-app-stderr.log").open("w")
process = subprocess.Popen([str(executable)], stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
try:
    report["application_launched_from_applications"] = True
    start = time.monotonic()
    while time.monotonic() - start < 120:
        assert process.poll() is None, "The installed app exited during startup."
        children = subprocess.run(["pgrep", "-P", str(process.pid)], capture_output=True, text=True)
        for value in children.stdout.split():
            pid = int(value)
            command = subprocess.check_output(["ps", "-ww", "-p", str(pid), "-o", "command="], text=True)
            if "llama-server" not in command:
                continue
            owned.add(pid)
            port = re.search(r"--port\s+(\d+)", command)
            key = re.search(r"--api-key\s+(\S+)", command)
            if not port or not key:
                continue
            # Keep the ephemeral key in memory; do not include it in test logs.
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
    assert report["automatic_ai_ready"], "Bundled AI did not start automatically."
    report["ai_startup_seconds"] = round(time.monotonic() - start, 2)
    subprocess.run(["osascript", "-e", f"tell application {json.dumps(str(app))} to quit"],
                   check=True, timeout=20, capture_output=True)
    report["clean_application_exit"] = process.wait(timeout=30) == 0
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        living = []
        for pid in owned:
            try:
                os.kill(pid, 0)
                living.append(pid)
            except ProcessLookupError:
                pass
        if not living:
            report["owned_ai_exited"] = True
            break
        time.sleep(0.2)
    assert report["clean_application_exit"] and report["owned_ai_exited"], "Owned AI remained after quitting the app."
    assert business_records(data_dir/"inventory.db")==expected
    auto_folder=Path.home()/"Inventory Manager Backups"
    copies=list(auto_folder.glob("inventory-auto-*.db"))
    assert copies and any(business_records(p)==expected for p in copies)
    report["existing_inventory_preserved_on_update"]=True
    report["windows_crlf_database_opened_on_installed_mac"]=True
    report["automatic_backup_outside_application_matches_inventory"]=True
    # Exercise the installed preference path, not a developer configuration.
    data_dir=Path.home()/"Library/Application Support"/info["CFBundleIdentifier"]/"data"
    settings_path=data_dir/"settings.json"
    preferences=json.loads(settings_path.read_text())
    for reason in ("manual","timed_out"):
        preferences.update(ai_enabled=False,ai_pause_reason=reason)
        settings_path.write_text(json.dumps(preferences))
        process=subprocess.Popen([str(executable)],stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr)
        deadline=time.monotonic()+5
        while time.monotonic()<deadline:
            assert process.poll() is None,"App must remain usable with AI off."
            children=subprocess.run(["pgrep","-P",str(process.pid)],capture_output=True,text=True)
            for value in children.stdout.split():
                pid=int(value)
                command=subprocess.check_output(["ps","-ww","-p",str(pid),"-o","command="],text=True)
                if "llama-server" in command or "voice-worker" in command:
                    owned.add(pid)
                    raise AssertionError("An AI/voice process was started despite saved off preferences.")
            time.sleep(.25)
        with sqlite3.connect(f"file:{data_dir/'inventory.db'}?mode=ro",uri=True) as database:
            assert database.execute("PRAGMA integrity_check").fetchone()[0]=="ok"
            database.execute("SELECT COUNT(*) FROM products").fetchone()
        subprocess.run(["osascript","-e",f"tell application {json.dumps(str(app))} to quit"],check=True,timeout=20,capture_output=True)
        assert process.wait(timeout=30)==0
        assert json.loads(settings_path.read_text())["ai_enabled"] is False
    assert business_records(data_dir/"inventory.db")==expected
    # Simulate a missing DB only in this disposable runner. Never overwrite the
    # prior fixture or its backup. Recovery history must survive repeated launch.
    (data_dir/"inventory.db").rename(root/".build/CI-PRESERVED-inventory.db")
    for suffix in ("-wal","-shm"):
        sidecar=data_dir/("inventory.db"+suffix)
        if sidecar.exists():sidecar.rename(root/(".build/CI-PRESERVED-inventory.db"+suffix))
    before={p.name for p in auto_folder.glob("inventory-auto-*.db")}
    for _ in range(2):
        process=subprocess.Popen([str(executable)],stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr)
        time.sleep(4)
        assert process.poll() is None
        assert (data_dir/"recovery-required.flag").is_file()
        assert {p.name for p in auto_folder.glob("inventory-auto-*.db")}==before
        subprocess.run(["osascript","-e",f"tell application {json.dumps(str(app))} to quit"],check=True,timeout=20,capture_output=True)
        assert process.wait(timeout=30)==0
    report["missing_database_history_preserved_across_relaunch"]=True
    report["saved_off_state_prevents_ai_and_voice_on_relaunch"]=True
    report["timeout_pause_persists_without_automatic_retry"]=True
    report["local_database_opens_with_ai_off"]=True
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        for pid in owned:
            try:
                os.kill(pid, 15)
            except ProcessLookupError:
                pass
    stdout.close()
    stderr.close()
    (root / ".build/MACOS-APP-SMOKE.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report))
