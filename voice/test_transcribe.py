"""Voice regressions use the build environment; no microphone or model needed."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, Mock, patch
import transcribe
import numpy as np


class OfflineWorkerTests(unittest.TestCase):
    def test_idle_start_and_shutdown_never_record(self):
        sd, model = Mock(), Mock()
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            transcribe.worker(sd, model, io.StringIO('{"shutdown":true}\n'))
        self.assertEqual(json.loads(output.getvalue()), {"ready": True})
        sd.rec.assert_not_called()
        model.transcribe.assert_not_called()

    def test_two_recordings_reuse_the_loaded_model_and_keep_ids(self):
        sd, model = Mock(), Mock()
        model.transcribe.return_value = ([SimpleNamespace(text=" أين الشاشة؟ ")], SimpleNamespace(language="ar"))
        output = io.StringIO()
        source = io.StringIO('{"id":"one","seconds":6}\n{"id":"two","seconds":2}\n')
        with contextlib.redirect_stdout(output):
            transcribe.worker(sd, model, source)
        rows = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(rows[0], {"ready": True})
        self.assertEqual(rows[1], {"id": "one", "text": "أين الشاشة؟", "language": "ar"})
        self.assertEqual(rows[2]["id"], "two")
        self.assertEqual(model.transcribe.call_count, 2)

    def test_bad_duration_and_microphone_error_do_not_crash_worker(self):
        sd, model = Mock(), Mock()
        sd.rec.side_effect = RuntimeError("No microphone")
        source = io.StringIO('{"id":"bad","seconds":100}\n{"id":"mic","seconds":6}\n')
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            transcribe.worker(sd, model, source)
        rows = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertIn("error", rows[1])
        self.assertEqual(rows[2]["error"], "No microphone")
        sd.rec.assert_called_once()

    def test_model_load_is_local_only_and_precedes_recording(self):
        sd, constructor = Mock(), Mock()
        sd.query_hostapis.return_value=[]
        with tempfile.TemporaryDirectory() as folder:
            for name in ("model.bin", "config.json", "tokenizer.json", "preprocessor_config.json"):
                Path(folder, name).write_bytes(b"fixture")
            with patch.dict("sys.modules", {"sounddevice": sd, "faster_whisper": SimpleNamespace(WhisperModel=constructor)}):
                result = transcribe.load_model(folder)
        self.assertIs(result[0], sd)
        self.assertTrue(constructor.call_args.kwargs["local_files_only"])
        self.assertEqual(constructor.call_args.kwargs["device"], "cpu")
        sd.rec.assert_not_called()

    def test_windows_default_wasapi_devices_support_whisper_sample_rate(self):
        sd,constructor=Mock(),Mock()
        sd.query_hostapis.return_value=[{'name':'Windows WASAPI','default_input_device':8,'default_output_device':9}]
        with tempfile.TemporaryDirectory() as folder:
            for name in ('model.bin','config.json','tokenizer.json'):Path(folder,name).write_bytes(b'fixture')
            with patch.object(transcribe.sys,'platform','win32'),patch.dict('sys.modules',{'sounddevice':sd,'faster_whisper':SimpleNamespace(WhisperModel=constructor)}):
                transcribe.load_model(folder)
        self.assertEqual(sd.default.device,(8,9))
        sd.WasapiSettings.assert_called_once_with(auto_convert=True)

    def test_continuous_turns_listen_then_speak_and_reuse_loaded_models(self):
        events = []
        def listen(sd, model, language):
            events.append(("listen", language))
            return {"text": "كم عدد الشاشات؟", "language": "ar"}
        def speak(sd, voices, text, language, on_progress=None, **kwargs):
            events.append(("speak", language, text))
            return {"spoken": True}
        requests = [
            {"id": "one", "action": "listen", "language": "ar"},
            {"id": "two", "action": "speak", "language": "ar", "text": "لدينا ثلاث شاشات."},
            {"id": "three", "action": "listen", "language": "en"},
            {"shutdown": True},
            {"id": "ignored", "action": "listen"},
        ]
        source = io.StringIO("\n".join(json.dumps(r) for r in requests))
        output = io.StringIO()
        with patch.object(transcribe, "listen", side_effect=listen), patch.object(transcribe, "speak", side_effect=speak), contextlib.redirect_stdout(output):
            transcribe.worker(Mock(), Mock(), source, {"ar": Mock(), "en": Mock()})
        self.assertEqual([event[0] for event in events], ["listen", "speak", "listen"])
        rows = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([row.get("id") for row in rows[1:]], ["one", "two", "three"])

    def test_playback_finishes_and_releases_output_before_next_turn(self):
        sd = MagicMock()
        voice = Mock()
        voice.synthesize.side_effect = lambda sentence: [SimpleNamespace(audio_float_array=np.arange(10, dtype=np.float32), sample_rate=24000)]
        stream = sd.OutputStream.return_value.__enter__.return_value
        finished = []
        with patch.object(transcribe.time, "sleep"):
            result = transcribe.speak(sd, {"en": voice}, "We have five screens. All five are available.", "en", on_progress=finished.append)
        self.assertTrue(result["spoken"])
        self.assertEqual(result["samples"], 20)
        self.assertEqual(finished, [10, 20])
        self.assertEqual(sd.OutputStream.call_count, 1)
        self.assertEqual([len(c.args[0]) for c in stream.write.call_args_list], [2400, 10, 10, 7200])
        np.testing.assert_equal(stream.write.call_args_list[-2].args[0][-1, 0], 9)
        sd.OutputStream.return_value.__exit__.assert_called_once()
        sd.InputStream.assert_not_called()
        sd.play.assert_not_called()
        stream.write.side_effect = RuntimeError("Speaker disconnected")
        with self.assertRaisesRegex(RuntimeError, "Speaker disconnected"):
            transcribe.speak(sd, {"en": voice}, "Try again.", "en")
        self.assertEqual(sd.OutputStream.return_value.__exit__.call_count, 2)

    def test_mixed_language_and_long_responses_do_not_lose_content(self):
        chunks = list(transcribe.speech_chunks("شاشة Samsung S3 متوفرة. OMR 5.000", "ar"))
        self.assertTrue(any(lang == "ar" and "سامسونج" in text for lang, text in chunks))
        self.assertIn(("en", "S3"), chunks)
        self.assertTrue(any(lang == "ar" and "ريال عماني" in text for lang, text in chunks))
        text = "مخزون " * 300
        chunks = list(transcribe.speech_chunks(text, "ar"))
        self.assertTrue(all(len(part) <= 350 for _, part in chunks))
        self.assertEqual(" ".join(part for _, part in chunks).split(), text.split())

    def test_decimal_prices_and_identifiers_stay_complete(self):
        for language in ("en", "ar"):
            chunks = list(transcribe.speech_chunks("Price 5.125. CR 1234. Samsung S3.", language))
            spoken = " ".join(text for _, text in chunks)
            self.assertIn("5.125", spoken)
            self.assertIn("1234", spoken)
            self.assertIn("S3", spoken)
            self.assertFalse(any(text in ("125", "000") for _, text in chunks))

    def test_spoken_money_keeps_exact_baisa_and_negative_sign(self):
        cases = [("OMR 5.125", "5 Omani rials and 125 baisa"), ("OMR 0.000", "0 Omani rials"),
                 ("OMR -0.001", "minus 1 baisa"), ("OMR 1,234.005", "1234 Omani rials and 5 baisa")]
        for text, expected in cases:
            self.assertEqual(" ".join(t for _, t in transcribe.speech_chunks(text, "en")), expected)
        self.assertEqual(" ".join(t for _, t in transcribe.speech_chunks("OMR 5.125", "ar")), "5 ريال عماني و 125 بيسة")

    def test_natural_pause_and_long_sentence_are_not_cut_off(self):
        # Continue after a 1.5 second hesitation and after the former 20s cap.
        self.assertFalse(transcribe.utterance_finished(24000, 0))
        self.assertTrue(transcribe.utterance_finished(32000, 0))
        frames = [np.ones((4096, 1), dtype=np.float32) for _ in range(82)]
        frames[42:48] = [np.zeros((4096, 1), dtype=np.float32) for _ in range(6)]
        frames.extend([np.zeros((4096, 1), dtype=np.float32) for _ in range(8)])
        sd, model = MagicMock(), Mock()
        sd.InputStream.return_value.__enter__.return_value.read.side_effect = [(f, False) for f in frames]
        model.transcribe.return_value = ([SimpleNamespace(text="The complete question", no_speech_prob=0)], SimpleNamespace(language="en"))
        def vad(audio, options):
            indices = np.flatnonzero(audio)
            return [{"start": int(indices[0]), "end": int(indices[-1]) + 1}] if len(indices) else []
        module = SimpleNamespace(get_speech_timestamps=vad, VadOptions=lambda **kw: kw)
        with patch.dict("sys.modules", {"faster_whisper.vad": module}):
            result = transcribe.listen(sd, model)
        self.assertEqual(result["text"], "The complete question")
        self.assertGreater(len(model.transcribe.call_args.args[0]), 22 * 16000)
        self.assertTrue(np.any(model.transcribe.call_args.args[0][48 * 4096:82 * 4096]))

    def test_worker_rejects_network_at_the_python_boundary(self):
        with self.assertRaisesRegex(RuntimeError, "does not permit network"):
            transcribe.offline_audit("socket.connect", ())


if __name__ == "__main__":
    unittest.main()
