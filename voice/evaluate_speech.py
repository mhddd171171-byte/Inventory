"""Local speech comparison. No microphone, network or customer data.

Run with the build environment after staging candidates in
.build/voice-model-evaluation. Recognition is a diagnostic, not a substitute
for a person listening to the accompanying WAV samples.
"""
import importlib.util
import json
import sys
import time
import subprocess
import wave
from pathlib import Path
import numpy as np
from faster_whisper import WhisperModel
from piper import PiperVoice
from nabra import NeuralVoice

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tmp/voice-v2.0.1-evaluation"
OUT.mkdir(parents=True, exist_ok=True)
spec = importlib.util.spec_from_file_location("previous_nabra", ROOT / ".build/voice-model-evaluation/previous/nabra.py")
previous = importlib.util.module_from_spec(spec)
spec.loader.exec_module(previous)
CASES = {
    "ar": [
        "لم أجد شاشة سامسونج في المخزون الحالي. يمكنك البحث بالاسم أو موديل الجهاز. رمز الصنف غير مطلوب للبحث.",
        "لإضافة منتج، افتح صفحة المنتجات، ثم أدخل رمز الصنف واسم المنتج والموديل، واضغط حفظ.",
        "قيمة المخزون خمسة ريالات عمانية. لدينا ثلاث شاشات وعشر بطاريات.",
        "هل تقصد الشاشة أم البطارية؟ أخبرني بالموديل الذي تبحث عنه، وسأتحقق من المخزون.",
    ],
    "en": [
        "I couldn't find a Samsung S3 screen in the current inventory. You can search by its name or device model. You don't need an item code to search.",
        "To add a product, open Products. Enter its item code, name and model, then choose Save. Use Stock Entry to record the quantity and purchase cost.",
        "The inventory value is five Omani rials. We have three screens and ten batteries.",
        "Do you mean the screen or the battery? Tell me the model you're looking for, and I'll check the inventory.",
    ],
}


def main():
    candidates = [
        ("ar", "previous-7m", lambda: previous.NabraVoice(ROOT / ".build/voice-model-evaluation/previous/tts/nabra")),
        ("ar", "nabra-82m-fp32", lambda: NeuralVoice(ROOT / ".build/voice-model-evaluation/ar", "ar", diacritizer="libtashkeel")),
        ("ar", "nabra-82m-catt", lambda: NeuralVoice(ROOT / ".build/voice-model-evaluation/ar", "ar")),
        ("en", "previous-piper", lambda: PiperVoice.load(str(ROOT / ".build/voice-model-evaluation/previous/tts/en_US-ljspeech-high.onnx"))),
        ("en", "kokoro-82m-q8", lambda: NeuralVoice(ROOT / ".build/voice-model-evaluation/en", "en", "model_q8.onnx")),
        ("en", "kokoro-82m-fp32", lambda: NeuralVoice(ROOT / ".build/voice-model-evaluation/en", "en")),
    ]
    if len(sys.argv) == 1:
        # Each candidate gets one eSpeak initialization. Its C bridge uses
        # process-global tables and cannot safely be initialized repeatedly.
        for _, name, _ in candidates:
            subprocess.run([sys.executable, __file__, name], check=True)
        rows = []
        for _, name, _ in candidates:
            rows.extend(json.loads((OUT / (name + ".json")).read_text(encoding="utf-8")))
        (OUT / "results.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
        return
    candidates = [c for c in candidates if c[1] == sys.argv[1]]
    model = WhisperModel(str(ROOT / "src-tauri/resources/voice/stt"), device="cpu", compute_type="int8", local_files_only=True, cpu_threads=4)
    rows = []
    for lang, name, constructor in candidates:
        started = time.perf_counter()
        voice = constructor()
        load_seconds = time.perf_counter() - started
        for index, text in enumerate(CASES[lang]):
            started = time.perf_counter()
            chunks = list(voice.synthesize(text))
            synthesis_seconds = time.perf_counter() - started
            rates = {chunk.sample_rate for chunk in chunks}
            assert len(rates) == 1
            rate = rates.pop()
            audio = np.concatenate([chunk.audio_float_array for chunk in chunks])
            assert len(audio) and np.isfinite(audio).all()
            path = OUT / f"{name}-{index}.wav"
            with wave.open(str(path), "wb") as wav:
                wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(rate)
                wav.writeframes((np.clip(audio, -1, 1) * 32767).astype("<i2").tobytes())
            # Resample diagnostic audio in memory; no encoded-media dependency.
            pcm = np.interp(np.arange(0, len(audio), rate / 16000), np.arange(len(audio)), audio).astype(np.float32)
            segments, _ = model.transcribe(pcm, language=lang, beam_size=5, vad_filter=False, condition_on_previous_text=False)
            recognized = " ".join(s.text.strip() for s in segments)
            row = dict(language=lang, model=name, case=index, text=text, recognized=recognized,
                       load_seconds=round(load_seconds, 3), synthesis_seconds=round(synthesis_seconds, 3),
                       audio_seconds=round(len(audio) / rate, 3), real_time_factor=round(synthesis_seconds / (len(audio) / rate), 3),
                       samples=len(audio), sample_rate=rate, wav=path.name)
            rows.append(row)
            print(json.dumps(row, ensure_ascii=True), flush=True)
        del voice
    (OUT / (sys.argv[1] + ".json")).write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
