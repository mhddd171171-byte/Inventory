"""Offline synthetic-speech integration evaluation. Does not use the microphone."""
import json
import time
import wave
from pathlib import Path
import transcribe

root = Path(__file__).resolve().parents[1]
resources = root / "src-tauri/resources/voice"
output = root / "tmp/voice-evaluation"
output.mkdir(parents=True, exist_ok=True)
started = time.perf_counter()
voices = transcribe.load_voices(resources)
_, model = transcribe.load_model(resources / "stt")
startup = time.perf_counter() - started
rows = []
for language, prompt in [
    ("en", "How many screens do we have in inventory?"),
    ("en", "What is our profit this month?"),
    ("ar", "كم عدد الشاشات الموجودة في المخزون؟"),
    ("ar", "ما هو إجمالي الربح لهذا الشهر؟"),
]:
    path = output / f"{language}-{len(rows)+1}.wav"
    start = time.perf_counter()
    with wave.open(str(path), "wb") as wav:
        voices[language].synthesize_wav(prompt, wav)
    synthesis = time.perf_counter() - start
    with wave.open(str(path), "rb") as wav:
        seconds = wav.getnframes() / wav.getframerate()
        assert seconds > 0.5 and wav.getnframes() > 0
    start = time.perf_counter()
    segments, info = model.transcribe(str(path), language=language, beam_size=3, vad_filter=True, condition_on_previous_text=False)
    recognized = " ".join(segment.text.strip() for segment in segments)
    row = {"language":language,"prompt":prompt,"transcript":recognized,"audio_seconds":seconds,"tts_seconds":synthesis,"stt_seconds":time.perf_counter()-start}
    rows.append(row)
    print(json.dumps(row, ensure_ascii=True), flush=True)
(output / "results.json").write_text(json.dumps({"startup_seconds":startup,"cases":rows}, indent=2, ensure_ascii=False), encoding="utf-8")
