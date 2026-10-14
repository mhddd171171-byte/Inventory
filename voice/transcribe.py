"""Packaged offline voice worker (GPL-3.0-or-later, alongside Piper).

Launched by the Rust backend from application resources when Voice Mode starts.
Its Python interpreter and complete local speech models are bundled. The worker
opens the microphone only for explicit Voice Mode listening or interruptible
reply playback. Audio stays in memory.
"""
import argparse
import json
import os
import sys
import time
import re
import threading
from pathlib import Path

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"


def offline_audit(event, args):
    if event in ("socket.connect", "socket.getaddrinfo", "socket.bind"):
        raise RuntimeError("The offline voice worker does not permit network connections.")


sys.addaudithook(offline_audit)
_OUTPUT_LOCK = threading.Lock()


def load_model(path):
    model_path = Path(path).resolve(strict=True)
    if not model_path.is_dir():
        raise ValueError("Choose a complete local faster-whisper model directory.")
    for name in ("model.bin", "config.json", "tokenizer.json"):
        if not (model_path / name).is_file():
            raise ValueError(f"Offline speech model is missing {name}; no download was attempted.")
    try:
        import sounddevice as sd
        from faster_whisper import WhisperModel
    except ImportError as exc:
        raise RuntimeError("A bundled voice dependency is missing. Repair the application installation, or type your question.") from exc
    if sys.platform == 'win32':
        # WASAPI follows the user's Windows default devices and avoids MME's
        # short capture buffer during concurrent local model inference.
        api=next((a for a in sd.query_hostapis() if a['name']=='Windows WASAPI'),None)
        if api and api['default_input_device']>=0 and api['default_output_device']>=0:
            sd.default.device=(api['default_input_device'],api['default_output_device'])
            # Shared-mode conversion permits Whisper's 16 kHz input on
            # default devices whose native mix format is 48 kHz.
            sd.default.extra_settings=sd.WasapiSettings(auto_convert=True)
    model = WhisperModel(str(model_path), device="cpu", compute_type="int8", local_files_only=True, cpu_threads=min(os.cpu_count() or 2, 4))
    return sd, model


def load_voices(resources):
    from nabra import NeuralVoice
    return {lang: NeuralVoice(Path(resources) / "tts" / folder, lang)
            for lang, folder in (("ar", "nabra"), ("en", "kokoro"))}


END_SILENCE_SAMPLES = 2 * 16000
MAX_UTTERANCE_SAMPLES = 90 * 16000
ASR_CONTEXT = 'Inventory. Item code. Stock. Screens. Batteries. Samsung. iPhone. المخزون. رمز الصنف. شاشات. بطاريات. سامسونج. آيفون.'


def utterance_finished(total, last_speech):
    # Ordinary pauses inside a sentence must not start the assistant's reply.
    return total - last_speech >= END_SILENCE_SAMPLES


def listen(sd, model, language="auto"):
    """Bounded listening outside reply playback; duplex handles interruptions."""
    import numpy as np
    from faster_whisper.vad import get_speech_timestamps, VadOptions
    if language not in ("auto", "en", "ar"):
        raise ValueError("Choose automatic, Arabic, or English speech recognition.")
    frames = []
    speech_started = False
    speech_start_sample = 0
    last_speech = 0
    total = 0
    started = time.monotonic()
    try:
        with sd.InputStream(samplerate=16000, channels=1, dtype="float32", blocksize=4096) as stream:
            while speech_started or time.monotonic() - started < 25:
                frame, overflow = stream.read(4096)
                if overflow:
                    raise RuntimeError("Microphone audio overflowed. Try again with other audio applications closed.")
                frames.append(frame[:, 0].copy())
                total += len(frame)
                # A rolling window bounds VAD processing without losing context.
                window = np.concatenate(frames[-12:])
                segments = get_speech_timestamps(window, VadOptions(min_speech_duration_ms=160, min_silence_duration_ms=400, speech_pad_ms=0))
                if segments:
                    if not speech_started:
                        speech_start_sample = total - len(window) + segments[0]["start"]
                    speech_started = True
                    last_speech = total - len(window) + segments[-1]["end"]
                if speech_started and utterance_finished(total, last_speech):
                    break
                if speech_started and total - speech_start_sample >= MAX_UTTERANCE_SAMPLES:
                    raise ValueError("Please pause after your question. I will wait for you to finish before replying (up to 90 seconds per turn).")
    except ValueError:
        raise
    except Exception as exc:
        raise RuntimeError(f"Microphone unavailable. Allow microphone access in system privacy settings and select a working default input device. {exc}") from exc
    if not speech_started:
        return {"text": "", "language": language, "no_speech": True}
    # Keep a short lead-in, not a long initial silence that can confuse STT.
    audio = np.concatenate(frames)[max(0, speech_start_sample - 6400):]
    segments, info = model.transcribe(audio, language=None if language == "auto" else language, beam_size=3, vad_filter=True,
                                     condition_on_previous_text=False, initial_prompt=ASR_CONTEXT)
    text = " ".join(segment.text.strip() for segment in segments if segment.no_speech_prob < 0.7).strip()
    if len(text) > 1000:
        raise ValueError("Please ask a shorter question.")
    return {"text": text, "language": info.language, "no_speech": not bool(text)}


def speech_chunks(text, preferred="auto"):
    # Pronunciation aliases are spoken only; product records/chat remain exact.
    arabic = preferred == "ar" or (preferred == "auto" and re.search(r"[\u0600-\u06ff]", text))
    text = re.sub(r"[\[\]*#`_]", "", text)
    def money(match):
        amount = match.group(1).replace(",", "")
        negative = amount.startswith("-")
        whole, fraction = amount.lstrip("-").split(".")
        rials, baisa = int(whole), int(fraction)
        units = []
        if rials or not baisa:
            units.append(f"{rials} " + ("ريال عماني" if arabic else "Omani rial" if rials == 1 else "Omani rials"))
        if baisa:
            units.append(f"{baisa} " + ("بيسة" if arabic else "baisa"))
        return ("سالب " if arabic else "minus ") * negative + (" و " if arabic else " and ").join(units)
    # Business money is integer baisa. Speak its exact value without reading
    # three decimal zeros or splitting the decimal into unrelated fragments.
    text = re.sub(r"\bOMR\s+(-?\d[\d,]*\.\d{3})(?!\d)", money, text)
    text = text.replace("OMR", "ريال عماني" if arabic else "Omani rials")
    if arabic:
        for source, spoken in [("Stock Entry", "إدخال المخزون"), ("Stock Out", "إخراج المخزون"),
                               ("Products", "المنتجات"), ("Reports", "التقارير"), ("Save", "حفظ"),
                               ("Samsung", "سامسونج"), ("iPhone", "آيفون"), ("Huawei", "هواوي"),
                               ("Xiaomi", "شاومي"), ("PDF", "بي دي إف"), ("Excel", "إكسل"),
                               ("CR", "سي آر"), ("SKU", "إس كيه يو")]:
            text = re.sub(r"(?<![A-Za-z])" + re.escape(source) + r"(?![A-Za-z])", spoken, text, flags=re.I)
    else:
        text = re.sub(r"\bCR\b", "C R", text)
        text = re.sub(r"\bSKU\b", "S K U", text)
    # Sentence punctuation only: decimal amounts such as 5.125 stay intact.
    for sentence in re.split(r"(?<=[.!?؟؛])\s+|\n+", text):
        if not sentence.strip():
            continue
        sections = re.split(r"([A-Za-z][A-Za-z0-9 .-]*[A-Za-z0-9]|[A-Za-z])", sentence) if re.search(r"[\u0600-\u06ff]", sentence) else [sentence]
        for section in sections:
            section = section.strip()
            if not section or not re.search(r"[\w\u0600-\u06ff]", section):
                continue
            language = "ar" if re.search(r"[\u0600-\u06ff]", section) or (arabic and not re.search(r"[A-Za-z]", section)) else "en"
            while len(section) > 350:
                cut = section.rfind(" ", 0, 350)
                if cut < 1:
                    cut = 350
                yield language, section[:cut].strip()
                section = section[cut:].strip()
            if section:
                yield language, section


def speak(sd, voices, text, language="auto", on_progress=None, model=None, barge_in=False, on_interrupt=None,input_language="auto"):
    if not isinstance(text, str) or not 1 <= len(text) <= 12000:
        raise ValueError("Speech text must contain 1 to 12,000 characters.")
    if language not in ("auto", "en", "ar"):
        raise ValueError("Unsupported speech language.")
    if input_language not in ("auto","en","ar"):
        raise ValueError("Unsupported input speech language.")
    import numpy as np
    from concurrent.futures import ThreadPoolExecutor
    parts = iter(speech_chunks(text, language))
    samples = 0
    def generate(part):
        lang, sentence = part
        chunks = list(voices[lang].synthesize(sentence))
        if not chunks or any(c.sample_rate != 24000 for c in chunks):
            raise RuntimeError("The bundled voice must produce complete 24 kHz speech.")
        return np.concatenate([c.audio_float_array for c in chunks]).astype(np.float32)
    first = next(parts, None)
    if first is None:
        raise ValueError("The response contains no speakable text.")
    if barge_in:
        from duplex import DuplexPlayback, BargeIn
        with DuplexPlayback(sd, on_interrupt) as stream:
            try:
                with ThreadPoolExecutor(max_workers=1) as executor:
                    pending = executor.submit(generate, first)
                    audio = stream.wait_until(pending)
                    stream.write(np.zeros((2400,1),dtype=np.float32))
                    while True:
                        following = next(parts,None)
                        pending = executor.submit(generate,following) if following else None
                        stream.write(audio, speech=True)
                        samples += len(audio)
                        if on_progress:
                            on_progress(samples)
                        if pending is None:
                            break
                        audio = stream.wait_until(pending)
                    stream.write(np.zeros((7200,1),dtype=np.float32))
                    # Monitor the echo tail before closing; users may start
                    # their next question immediately as the reply finishes.
                    time.sleep(0.35)
                    stream.check()
            except BargeIn:
                pcm = stream.interruption_audio()
                if model is None:
                    raise RuntimeError("Local transcription model is unavailable.")
                # This capture already has a detected start/end and echo
                # removal. A second VAD pass can drop the first short word.
                segments,info = model.transcribe(pcm,language=None if input_language=="auto" else input_language,beam_size=5,vad_filter=False,condition_on_previous_text=False,initial_prompt=ASR_CONTEXT)
                transcript = " ".join(s.text.strip() for s in segments if s.no_speech_prob<0.7).strip()
                if len(transcript)>1000:
                    raise ValueError("Please ask a shorter question.")
                return {"spoken":False,"interrupted":True,"samples":stream.speech_samples//2,
                        "characters":len(text),"arabic_characters":len(re.findall(r"[\u0621-\u064a]",text)),
                        "transcript":{"text":transcript,"language":info.language,"no_speech":not bool(transcript)}}
        return {"spoken":True,"samples":samples,"characters":len(text),
                "arabic_characters":len(re.findall(r"[\u0621-\u064a]",text))}
    # Prefetch the next phrase while this one plays. One output stream avoids
    # device restart clipping, sample-rate switches and gaps inside a reply.
    with ThreadPoolExecutor(max_workers=1) as executor:
        pending = executor.submit(generate, first)
        audio = pending.result()
        with sd.OutputStream(samplerate=24000, channels=1, dtype="float32", blocksize=1024) as stream:
            stream.write(np.zeros((2400, 1), dtype=np.float32))
            while True:
                following = next(parts, None)
                pending = executor.submit(generate, following) if following else None
                stream.write(audio.reshape(-1, 1))
                samples += len(audio)
                if on_progress:
                    on_progress(samples)
                if pending is None:
                    break
                audio = pending.result()
            # PortAudio's normal stream stop drains queued audio. Keep the
            # final consonant/reverb tail before closing and reopening input.
            stream.write(np.zeros((7200, 1), dtype=np.float32))
    time.sleep(0.55)
    return {"spoken": True, "samples": samples, "characters": len(text),
            "arabic_characters": len(re.findall(r"[\u0621-\u064a]", text))}


def transcribe(sd, model, seconds):
    if type(seconds) is not int or not 2 <= seconds <= 20:
        raise ValueError("Recording must be between 2 and 20 seconds.")
    audio = sd.rec(seconds * 16000, samplerate=16000, channels=1, dtype="float32")
    try:
        sd.wait()
    except Exception:
        sd.stop()
        raise
    segments, info = model.transcribe(audio.reshape(-1), beam_size=3, vad_filter=False, condition_on_previous_text=False)
    text = " ".join(segment.text.strip() for segment in segments).strip()
    return {"text": text, "language": info.language}


def emit(value):
    with _OUTPUT_LOCK:
        print(json.dumps(value, ensure_ascii=True), flush=True)


def worker(sd, model, source, voices=None):
    emit({"ready": True})
    while True:
        line = source.readline(65537)
        if not line:
            return
        if len(line) > 65536:
            raise ValueError("Speech request exceeded the permitted size.")
        request = json.loads(line)
        if not isinstance(request, dict):
            raise ValueError("Invalid speech request.")
        if request.get("shutdown") is True:
            return
        request_id = request.get("id")
        if not isinstance(request_id, str) or not 1 <= len(request_id) <= 64:
            raise ValueError("Invalid recording identifier.")
        try:
            action = request.get("action", "transcribe")
            if action == "listen":
                result = listen(sd, model, request.get("language", "auto"))
            elif action == "speak":
                if not voices:
                    raise RuntimeError("The bundled speech synthesis voices are unavailable.")
                progress = lambda samples: emit({"id": request_id, "progress": True, "played_samples": samples})
                interrupt = lambda samples: emit({"id":request_id,"progress":True,"interrupted":True,"played_samples":samples})
                result = speak(sd, voices, request.get("text"), request.get("language", "auto"), on_progress=progress,
                               model=model,barge_in=True,on_interrupt=interrupt,input_language=request.get("input_language","auto"))
            elif action == "transcribe":
                result = transcribe(sd, model, request.get("seconds"))
            else:
                raise ValueError("Unsupported voice operation.")
            emit({"id": request_id, **result})
        except Exception as exc:
            emit({"id": request_id, "error": str(exc)})


def main():
    # Rust writes UTF-8 JSON. Redirected Python input on Windows otherwise uses
    # the system ANSI code page, turning Arabic into unreadable Latin characters.
    sys.stdin.reconfigure(encoding="utf-8", errors="strict")
    sys.stdout.reconfigure(encoding="utf-8", errors="strict")
    parser = argparse.ArgumentParser()
    parser.add_argument("--model")
    parser.add_argument("--resources")
    parser.add_argument("--worker", action="store_true")
    parser.add_argument("--seconds", type=int)
    args = parser.parse_args()
    if not args.model and not args.resources:
        raise ValueError("Bundled voice resources are unavailable.")
    sd, model = load_model(args.model or Path(args.resources) / "stt")
    voices = load_voices(args.resources) if args.resources else None
    if args.resources:
        from faster_whisper.vad import get_vad_model
        get_vad_model()  # Preload the bundled Silero graph without opening a microphone.
    if args.worker:
        worker(sd, model, sys.stdin, voices)
    else:
        emit(transcribe(sd, model, args.seconds))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        emit({"error": str(exc)})
        sys.exit(1)
