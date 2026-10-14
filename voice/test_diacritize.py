"""Pronunciation must never change database facts, numbers or base letters."""
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
import numpy as np
import diacritize


class ArabicPronunciationTests(unittest.TestCase):
    def test_pronunciation_aliases_only_add_vowels(self):
        for base, spoken in diacritize.PRONUNCIATIONS.items():
            self.assertEqual(re.sub(r"[\u064b-\u0652\u0670]", "", spoken), base)
        spoken = diacritize.pronunciation_text("سَامِسُونَجَ، الصِّنْفِ. 5.125 CR 1234")
        self.assertIn("سَامْسُونْج", spoken)
        self.assertIn("الصِّنْف.", spoken)
        self.assertIn("5.125 CR 1234", spoken)

    def test_model_tags_preserve_punctuation_amounts_and_all_letters(self):
        text = "سعر 5.125، رمز 1234."
        with tempfile.TemporaryDirectory() as folder:
            config = {"letters": {" ": 3, "s": 4}, "arabic_to_bw": {"س": "s"}, "diacritics": ["", "َ"]}
            Path(folder, "tokenizer.json").write_text(json.dumps(config), encoding="utf-8")
            session = Mock()
            session.run.return_value = [np.tile([0, 1], (1, len(text), 1)).astype(np.float32)]
            with patch.object(diacritize.ort, "InferenceSession", return_value=session):
                output = diacritize.ArabicDiacritizer(folder)(text)
            self.assertEqual(re.sub(r"[\u064b-\u0652\u0670]", "", output), text)
            self.assertIn("5.125", output)
            self.assertIn("1234", output)


if __name__ == "__main__":
    unittest.main()
