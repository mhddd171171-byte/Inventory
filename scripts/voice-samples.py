"""Generate listening samples through the production speech pipeline, offline.

These are fixed application-help examples, not invented customer inventory.
Captures the output stream in memory without opening audio devices.
"""
import json
import sys
import wave
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "voice"))
import transcribe


class CaptureStream:
    def __init__(self, **options):
        assert options["samplerate"] == 24000 and options["channels"] == 1
        self.frames = []
        self.closed = False

    def __enter__(self):
        return self

    def write(self, pcm):
        self.frames.append(pcm.reshape(-1).copy())

    def __exit__(self, *args):
        self.closed = True


class CaptureAudio:
    def OutputStream(self, **options):
        self.output = CaptureStream(**options)
        return self.output


def main():
    voices = transcribe.load_voices(ROOT / "src-tauri/resources/voice")
    folder = ROOT / "deliverables/voice-samples"
    folder.mkdir(exist_ok=True)
    samples = [
        ("ar", "مرحبًا. يمكنك البحث عن شاشة Samsung S3 بالاسم أو بالموديل. لا تحتاج إلى رمز الصنف للبحث. لإضافة منتج، افتح Products، ثم أدخل رمز الصنف واسم المنتج والموديل، واضغط Save."),
        ("en", "Hi. You can search for a Samsung S3 screen by name or device model. You don't need an item code to search. To add a product, open Products. Enter its item code, name and model, then choose Save."),
        ("ar", "قيمة المخزون بسعر الشراء: OMR 5.125. يمكنك تسجيل الكمية والتكلفة في Stock Entry."),
    ]
    metadata = []
    for index, (lang, text) in enumerate(samples):
        capture = CaptureAudio()
        progress = []
        result = transcribe.speak(capture, voices, text, lang, progress.append)
        assert capture.output.closed and progress[-1] == result["samples"]
        audio = np.concatenate(capture.output.frames)
        path = folder / (f"{lang}-conversation.wav" if index < 2 else "ar-money-example.wav")
        with wave.open(str(path), "wb") as wav:
            wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(24000)
            wav.writeframes((np.clip(audio, -1, 1) * 32767).astype("<i2").tobytes())
        metadata.append(dict(language=lang, text=text, file=path.name, seconds=len(audio) / 24000, **result))
    (folder / "SAMPLES.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(metadata, ensure_ascii=True), flush=True)


if __name__ == "__main__":
    main()
