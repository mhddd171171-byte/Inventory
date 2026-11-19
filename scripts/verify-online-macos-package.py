"""Inspect the final mounted DMG and its real installed application."""
import hashlib
import json
import plistlib
import runpy
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
dmg = next((root / "src-tauri/target/x86_64-apple-darwin/release/bundle/dmg").glob("*.dmg"))
subprocess.run(["hdiutil", "verify", str(dmg)], check=True)
mount = root / ".build/online-package-mount"
mount.mkdir(exist_ok=True)
report = {"version": json.loads((root / "package.json").read_text())["version"],
          "native_intel": True, "customer_mac_tested": False, "live_turso_on_mac_tested": False}
subprocess.run(["hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", str(mount), str(dmg)], check=True)
try:
    app = next(mount.glob("*.app"))
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert info["CFBundleShortVersionString"] == report["version"]
    assert info["LSMinimumSystemVersion"] == "13.0" and info["NSMicrophoneUsageDescription"]
    resources = app / "Contents/Resources/resources"
    models=runpy.run_path(str(root / "scripts/verify-macos-models.py"))["verified_models"]()
    for model in models:
        relative=Path(model["path"]).relative_to("src-tauri/resources")
        with (resources / relative).open("rb") as stream:
            assert hashlib.file_digest(stream,"sha256").hexdigest()==model["sha256"]
    report["verified_bundled_models"]=len(models)
    assert not list(resources.rglob("*.exe")) and not list(resources.rglob("*.dll"))
    for binary in [app / "Contents/MacOS" / info["CFBundleExecutable"],
                   resources / "ai/llama-server", resources / "voice/runtime/voice-worker"]:
        assert subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip() == "x86_64"
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    worker = subprocess.run([str(resources / "voice/runtime/voice-worker"), "--worker", "--resources", str(resources / "voice")],
                            input='{"shutdown":true}\n', text=True, capture_output=True, timeout=360)
    assert worker.returncode == 0, worker.stderr[-2000:]
    assert any(json.loads(line).get("ready") is True for line in worker.stdout.splitlines() if line.startswith("{"))
    report["bundled_voice_worker_initialized"] = True
    subprocess.run([sys.executable, str(root / "scripts/smoke-macos-online-app.py"), str(app)], check=True)
    report["installed_app_and_local_ai_passed"] = True
    report["passed"] = True
finally:
    subprocess.run(["hdiutil", "detach", str(mount)], check=False)
    (root / ".build/MACOS-ONLINE-PACKAGE.json").write_text(json.dumps(report, indent=2))
with dmg.open("rb") as stream:
    digest = hashlib.file_digest(stream, "sha256").hexdigest()
(root / ".build/MACOS-SHA256SUMS.txt").write_text(f"{digest}  {dmg.name}\n")
print(json.dumps(report))
