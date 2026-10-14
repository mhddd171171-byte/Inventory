"""Regression tests for pronunciation initialization and complete phonemes."""
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch
import numpy as np
import nabra


class NeuralVoiceTests(unittest.TestCase):
    def test_two_languages_initialize_espeak_only_once_and_use_correct_style_row(self):
        with tempfile.TemporaryDirectory() as folder:
            Path(folder, "vocab.json").write_text(json.dumps({"a": 43, "b": 44, "c": 45}), encoding="utf-8")
            session = Mock()
            session.get_inputs.return_value = [SimpleNamespace(name=n) for n in ("input_ids", "ref_s", "speed")]
            session.run.return_value = [np.array([0.1, 0.2], dtype=np.float32)]
            phones = Mock()
            phones.phonemize.return_value = [list("abc")]
            styles = np.tile(np.arange(510)[:, None], (1, 256)).astype(np.float32)
            with patch.object(nabra, "_PHONEMIZER", None), patch.object(nabra, "EspeakPhonemizer", return_value=phones) as constructor, patch.object(nabra.ort, "InferenceSession", return_value=session), patch.object(nabra.np, "fromfile", return_value=styles), patch.object(nabra, "TashkeelDiacritizer", return_value=lambda text: text):
                arabic = nabra.NeuralVoice(folder, "ar", diacritizer="libtashkeel")
                english = nabra.NeuralVoice(folder, "en")
                list(arabic.synthesize("Arabic")); list(english.synthesize("English")); list(arabic.synthesize("Arabic again"))
                constructor.assert_called_once()
                self.assertEqual([c.args[0] for c in phones.phonemize.call_args_list], ["ar", "en-us", "ar"])
                feed = session.run.call_args.args[1]
                np.testing.assert_equal(feed["input_ids"], [[0, 43, 44, 45, 0]])
                self.assertEqual(feed["ref_s"][0, 0], 3)

    def test_long_phonemes_preserve_every_word_in_order(self):
        phones = " ".join(["abcd" * 19] * 24) + "."
        parts = list(nabra.phoneme_chunks(phones))
        self.assertGreater(len(parts), 1)
        self.assertTrue(all(len(part) <= 450 for part in parts))
        self.assertEqual(" ".join(parts), phones)
        with self.assertRaises(ValueError):
            list(nabra.phoneme_chunks("a" * 451))


if __name__ == "__main__":
    unittest.main()
