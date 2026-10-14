"""Synthesize real bilingual speech on the build Mac, without audio hardware."""
import json
import platform
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
if platform.system() != "Darwin" or platform.machine() != "x86_64":
    raise SystemExit("Native Intel Mac speech verification requires macOS.")
sys.path.insert(0, str(root / "voice"))
from transcribe import load_voices

voices = load_voices(root / ".build/macos-resources/voice")
rows = []
for language, text in (("ar", "مرحباً، كيف أساعدك في المخزون؟"), ("en", "Hi. How can I help with your inventory?")):
    chunks = list(voices[language].synthesize(text))
    samples = sum(len(chunk.audio_float_array) for chunk in chunks)
    assert samples > 24000, "Speech must contain a complete audible response."
    assert all(chunk.sample_rate == 24000 for chunk in chunks)
    rows.append({"language": language, "samples": samples, "seconds": samples / 24000})
(root / ".build/MACOS-SPEECH-SMOKE.json").write_text(json.dumps({"passed": True, "responses": rows,
    "audio_hardware_tested": False}, indent=2), encoding="utf-8")
print(json.dumps(rows))
