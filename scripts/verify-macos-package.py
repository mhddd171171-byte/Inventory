"""Verify the real DMG's mounted app/resources and native offline worker."""
import hashlib
import json
import platform
import plistlib
import runpy
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
if platform.system() != "Darwin" or platform.machine() != "x86_64":
    raise SystemExit("Package verification requires the native Intel macOS runner.")
dmg_dir = root / "src-tauri/target/x86_64-apple-darwin/release/bundle/dmg"
dmgs = list(dmg_dir.glob("*.dmg"))
assert len(dmgs) == 1, "Expected exactly one Intel DMG."
dmg = dmgs[0]
subprocess.run(["hdiutil", "verify", str(dmg)], check=True)
mount = root / ".build/macos-verification-mount"
mount.mkdir(exist_ok=True)
report = {"native_mac_build_performed": True, "architecture": platform.machine(),
          "runner_macos": platform.mac_ver()[0], "dmg": dmg.name,
          "customer_hardware_tested": False, "microphone_speaker_tested": False}
try:
    subprocess.run(["hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", str(mount), str(dmg)], check=True)
    apps = list(mount.glob("*.app"))
    assert len(apps) == 1
    app = apps[0]
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    resources = app / "Contents/Resources/resources"
    assert (resources / "voice/espeak-ng-data/phontab").is_file()
    assert info["NSMicrophoneUsageDescription"]
    assert info["LSMinimumSystemVersion"] == "13.0"
    models = runpy.run_path(str(root / "scripts/verify-macos-models.py"))["verified_models"]()
    for record in models:
        relative = Path(record["path"]).relative_to("src-tauri/resources")
        with (resources / relative).open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == record["sha256"], str(relative)
    report["verified_model_files"] = len(models)
    executables = [app / "Contents/MacOS" / info["CFBundleExecutable"],
                   resources / "ai/llama-server", resources / "voice/runtime/voice-worker"]
    for executable in executables:
        result = subprocess.check_output(["lipo", "-archs", str(executable)], text=True).strip()
        assert "x86_64" in result.split(), f"Wrong architecture: {executable} {result}"
    assert not list(resources.rglob("*.exe")), "Windows executables entered the Mac bundle."
    assert not list(resources.rglob("*.dll")), "Windows libraries entered the Mac bundle."
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    signature = subprocess.run(["codesign", "-dv", "--verbose=2", str(app)], capture_output=True, text=True, check=True)
    report["developer_id_signed"] = "Authority=Developer ID Application" in signature.stderr
    # Startup loads Whisper, both voices and local VAD, without a microphone or downloads.
    worker = subprocess.run([str(executables[2]), "--worker", "--resources", str(resources / "voice")],
        input='{"shutdown":true}\n', capture_output=True, text=True, timeout=360)
    assert worker.returncode == 0, worker.stderr[-6000:]
    lines = [json.loads(line) for line in worker.stdout.splitlines() if line.startswith("{")]
    assert any(line.get("ready") is True for line in lines), worker.stderr[-6000:]
    report["mounted_dmg_voice_worker_initialized"] = True
    subprocess.run(["python3.12", str(root / "scripts/smoke-macos-app.py"), str(app)], check=True)
    report["installed_app_automatic_ai_startup_and_shutdown"] = True
    report["passed"] = True
finally:
    subprocess.run(["hdiutil", "detach", str(mount)], check=False)
    (root / ".build/MACOS-PACKAGE-VERIFICATION.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
with dmg.open("rb") as stream:
    digest = hashlib.file_digest(stream, "sha256").hexdigest()
(root / ".build/MACOS-SHA256SUMS.txt").write_text(f"{digest}  {dmg.name}\n", encoding="utf-8")
print(json.dumps(report))
