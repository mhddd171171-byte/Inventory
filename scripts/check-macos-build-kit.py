"""Read-only verification of the Mac transfer archive, not native Mac testing."""
import hashlib
import json
import plistlib
import sys
import tarfile
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
version = json.loads((root / "package.json").read_text())["version"]
path = Path(sys.argv[1]) if len(sys.argv) > 1 else root / f"deliverables/macOS-build-kit/InventoryManager-{version}-IntelMac-BUILD-KIT.zip"
with zipfile.ZipFile(path) as archive:
    names = archive.namelist()
    assert len(names) == len(set(names)), "Duplicate archive paths"
    assert archive.testzip() is None, "ZIP integrity failed"
    for name in names:
        assert not Path(name).is_absolute() and ".." not in Path(name).parts, name
        assert Path(name).suffix.lower() not in {".exe", ".dll", ".db", ".p12", ".key"}, name
        assert "runtime" not in Path(name).parts, "Native Windows worker must not enter Mac build kit"
    manifest = json.loads(archive.read("MACOS-BUILD-MANIFEST.json"))
    assert manifest["native_mac_build_performed"] is False
    for model in manifest["models"]:
        with archive.open(model["path"]) as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == model["sha256"], model["path"]
        assert archive.getinfo(model["path"]).file_size == model["bytes"]
    record = json.loads(archive.read("build-inputs/llama-b11554.tar.json"))
    with archive.open("build-inputs/llama-b11554.tar.gz") as stream:
        assert hashlib.file_digest(stream, "sha256").hexdigest() == record["sha256"]
    with archive.open("build-inputs/llama-b11554.tar.gz") as stream:
        with tarfile.open(fileobj=stream, mode="r|gz") as source:
            assert any(member.name == "llama.cpp-b11554/CMakeLists.txt" for member in source)
    config = json.loads(archive.read("src-tauri/tauri.macos.conf.json"))
    assert config["bundle"]["targets"] == ["app", "dmg"]
    assert config["bundle"]["resources"]["../.build/macos-resources/licenses/"] == "resources/licenses/"
    info = plistlib.loads(archive.read("src-tauri/Info.plist"))
    assert info["NSMicrophoneUsageDescription"]
    entitlements = plistlib.loads(archive.read("src-tauri/Entitlements.plist"))
    assert entitlements["com.apple.security.device.audio-input"] is True
print(f"PASS: {len(names)} unique entries, {len(manifest['models'])} model hashes, llama source hash/layout, resource configuration and microphone declarations.")
print("Native Mac compilation, signing, installed behavior and hardware performance remain unverified.")
