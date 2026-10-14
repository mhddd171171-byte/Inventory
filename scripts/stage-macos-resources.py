"""Stage platform-neutral models and freshly built Intel Mac runtimes only."""
import json
import shutil
import platform
import uuid
import runpy
import piper.phonemize_espeak
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if platform.system() != "Darwin" or platform.machine() != "x86_64":
    raise SystemExit("Resource staging requires the Intel macOS build host.")
runpy.run_path(str(ROOT / "scripts/verify-macos-models.py"))["verified_models"]()
stage = ROOT / ".build/macos-resources"
# Do not retain stale helper libraries from a previous packaging attempt.
if stage.exists():
    if stage.is_symlink() or stage.resolve().parent != (ROOT / ".build").resolve():
        raise SystemExit("Unsafe resource staging directory.")
    stage.rename(stage.with_name("macos-resources.previous-" + uuid.uuid4().hex))
source = ROOT / "src-tauri/resources"

def copy_model(source_path, destination):
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        destination.hardlink_to(source_path)
    except OSError:
        shutil.copy2(source_path, destination)
    return str(destination)

ai = stage / "ai"
ai.mkdir(parents=True, exist_ok=True)
manifest = json.loads((source / "ai/runtime.json").read_text())
name = manifest["model_file"]
if Path(name).name != name:
    raise SystemExit("Model must be a single resource filename.")
copy_model(source / "ai" / name, ai / name)
manifest.update(context_size=2048, cpu_threads=4)
(ai / "runtime.json").write_text(json.dumps(manifest, indent=2))
shutil.copy2(ROOT / ".build/llama-macos/bin/llama-server", ai / "llama-server")
for name in ("LICENSE-Qwen3.txt", "LICENSE-llama.cpp.txt"):
    shutil.copy2(source / "ai" / name, ai / name)
voice = stage / "voice"
for name in ("stt", "tts"):
    shutil.copytree(source / "voice" / name, voice / name, copy_function=copy_model)
shutil.copy2(source / "voice/MODELS.json", voice / "MODELS.json")
# Keep tables as ordinary resource files: Tauri does not preserve every
# PyInstaller data-directory symlink when copying a nested helper bundle.
shutil.copytree(piper.phonemize_espeak.ESPEAK_DATA_DIR, voice / "espeak-ng-data", symlinks=False)
assert (voice / "espeak-ng-data/phontab").is_file()
shutil.copytree(ROOT / ".build/voice-macos-dist/voice-worker", voice / "runtime", dirs_exist_ok=True, symlinks=True)
shutil.copytree(source / "licenses", stage / "licenses", ignore=shutil.ignore_patterns("voice"))
cached_sources = source / "licenses/voice/sources"
if cached_sources.is_dir():
    shutil.copytree(cached_sources, stage / "licenses/voice/sources")
print(stage)
