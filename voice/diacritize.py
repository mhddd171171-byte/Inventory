"""Offline CATT-EO int8 inference (Abjad AI model/tokenizer, Apache-2.0).

Character/tag maps are derived from the pinned upstream tokenizer. Unlike
text-cleaning decoders, this adapter retains punctuation, amounts, model
numbers and every original base letter; predictions can only add diacritics.
"""
import json
import os
import re
from pathlib import Path
import numpy as np
import onnxruntime as ort

# Stable app vocabulary/proper names that general diacritizers often mistake.
# These only supply vowels: base letters and all business data stay unchanged.
PRONUNCIATIONS = {
    "سامسونج": "سَامْسُونْج", "آيفون": "آيْفُون", "هواوي": "هُوَاوِي", "شاومي": "شَاوْمِي",
    "الموديل": "الْمُودِيل", "موديل": "مُودِيل", "الصنف": "الصِّنْف", "حفظ": "حِفْظ",
    "واضغط": "وَاضْغَطْ", "اضغط": "اِضْغَطْ", "بالاسم": "بِالِاسْم",
}


def pronunciation_text(text):
    def word(match):
        original = match.group()
        base = re.sub(r"[\u064b-\u0652\u0670]", "", original)
        return PRONUNCIATIONS.get(base, original)
    text = re.sub(r"[\u0621-\u064a\u064b-\u0652\u0670]+", word, text)
    # Read sentence endings with a natural Arabic pause (waqf).
    return re.sub(r"[\u064b-\u0650\u0652]+(?=\s*[,،.!?؟؛:;]|\s*$)", "", text)


class ArabicDiacritizer:
    def __init__(self, folder):
        folder = Path(folder)
        config = json.loads((folder / "tokenizer.json").read_text(encoding="utf-8"))
        self.letters = config["letters"]
        self.mapping = config["arabic_to_bw"]
        self.tags = config["diacritics"]
        options = ort.SessionOptions()
        options.intra_op_num_threads = min(os.cpu_count() or 2, 4)
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(str(folder / "catt_eo.int8.onnx"), sess_options=options, providers=["CPUExecutionProvider"])

    def __call__(self, text):
        base = re.sub(r"[\u064b-\u0652\u0670]", "", text).replace("ٱ", "ا")
        if not base:
            return base
        if len(base) > 500:
            raise ValueError("Split long speech on word boundaries before Arabic diacritization.")
        # Non-Arabic positions become spaces to the model, then return unchanged.
        ids = [self.letters.get(self.mapping.get(c, " "), self.letters[" "]) for c in base]
        values = np.array([ids], dtype=np.int64)
        logits = self.session.run(None, {"src": values, "src_mask": np.ones((1, 1, len(ids), len(ids)), dtype=bool)})[0]
        predictions = np.argmax(logits, axis=-1).reshape(-1)
        if len(predictions) != len(base):
            raise RuntimeError("Arabic pronunciation output did not preserve the input length.")
        diacritized = "".join(c + (self.tags[int(tag)] if c in self.mapping and self.mapping[c] in self.letters and c != " " else "")
                             for c, tag in zip(base, predictions))
        return pronunciation_text(diacritized)
