"""Offline Nabra-82M/Kokoro-82M ONNX adapter (Apache-2.0 models).
Arabic cleanup follows Oddadmix's Arabic G2P conventions. Uses Piper's
bundled eSpeak and diacritizer; never fetches resources at runtime.
"""
import json
import os
import re
import threading
from pathlib import Path
from types import SimpleNamespace
import numpy as np
import onnxruntime as ort
from piper.phonemize_espeak import EspeakPhonemizer
from piper.tashkeel import TashkeelDiacritizer

_PHONEMIZER = None
_PHONEMIZER_LOCK = threading.Lock()


def phoneme_chunks(phones, limit=450):
    """Keep words intact and every phoneme below the 510-token style limit."""
    pending = ""
    for word in phones.split(" "):
        if len(word) > limit:
            raise ValueError("A spoken word is too long. Please use a shorter product description.")
        candidate = pending + (" " if pending else "") + word
        if len(candidate) > limit:
            yield pending
            pending = word
        else:
            pending = candidate
    if pending.strip():
        yield pending


class NeuralVoice:
    sample_rate = 24000

    def __init__(self, folder, language="ar", model_file="model.onnx", diacritizer="catt"):
        folder = Path(folder)
        self.language = language
        options = ort.SessionOptions()
        options.intra_op_num_threads = min(os.cpu_count() or 2, 4)
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(str(folder / model_file), sess_options=options, providers=["CPUExecutionProvider"])
        config = json.loads((folder / "vocab.json").read_text(encoding="utf-8"))
        self.vocab = config.get("vocab", config)
        if language == "ar":
            self.vocab.update({"ʕ": 7, "ħ": 8})
        self.styles = np.fromfile(folder / "voice.bin", dtype="<f4").reshape(510, 256)
        names = {i.name for i in self.session.get_inputs()}
        self.style_input = "ref_s" if "ref_s" in names else "style"
        if names != {"input_ids", self.style_input, "speed"}:
            raise ValueError("The bundled speech model has an incompatible ONNX interface.")
        # eSpeak holds process-global tables. Initializing it once per voice
        # corrupts those tables when Arabic and English voices are both loaded.
        global _PHONEMIZER
        with _PHONEMIZER_LOCK:
            if _PHONEMIZER is None:
                _PHONEMIZER = EspeakPhonemizer()
        self.phonemizer = _PHONEMIZER
        self.diacritizer = None
        if language == "ar":
            if diacritizer == "catt":
                from diacritize import ArabicDiacritizer
                self.diacritizer = ArabicDiacritizer(folder.parent / "catt")
            else:
                self.diacritizer = TashkeelDiacritizer()

    def synthesize(self, text):
        if self.diacritizer:
            text = self.diacritizer(text)
        with _PHONEMIZER_LOCK:
            sentences = self.phonemizer.phonemize("ar" if self.language == "ar" else "en-us", text)
        for sentence in sentences:
            phones = "".join(sentence)
            phones = re.sub(r"(?<=\S)\.(?=\S)", "", phones)
            for a, b in [("dʒ", "ʤ"), ("d͡ʒ", "ʤ"), ("tʃ", "ʧ"), ("t͡ʃ", "ʧ"), ("̪", ""), ("ˤ", ""), ("[", ""), ("]", ""), ("{", ""), ("}", ""), ("-", "")]:
                phones = phones.replace(a, b)
            unknown = set(phones) - self.vocab.keys()
            if unknown:
                raise ValueError("The speech phonemizer returned unsupported sounds: " + repr(sorted(unknown)))
            for part in phoneme_chunks(phones):
                tokens = [self.vocab[c] for c in part]
                if not tokens:
                    continue
                # Voice-pack row is the token count, excluding BOS/EOS (not n-1).
                audio = self.session.run(None, {"input_ids": np.array([[0, *tokens, 0]], dtype=np.int64),
                    self.style_input: self.styles[len(tokens)][None, :],
                    "speed": np.array([0.96 if self.language == "ar" else 1.0], dtype=np.float32)})[0].reshape(-1).astype(np.float32)
                if not len(audio) or not np.isfinite(audio).all():
                    raise RuntimeError("The speech model did not produce valid audio.")
                yield SimpleNamespace(audio_float_array=np.clip(audio, -1, 1), sample_rate=self.sample_rate)

    def synthesize_wav(self, text, wav):
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(self.sample_rate)
        for chunk in self.synthesize(text):
            wav.writeframes((np.clip(chunk.audio_float_array,-1,1)*32767).astype("<i2").tobytes())


class NabraVoice(NeuralVoice):
    def __init__(self, folder):
        super().__init__(folder, "ar")
