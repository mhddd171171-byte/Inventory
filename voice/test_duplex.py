import unittest
from unittest.mock import Mock,patch
from types import SimpleNamespace
import numpy as np
import duplex
import transcribe

class InterruptionTests(unittest.TestCase):
    def test_silence_and_a_single_click_do_not_interrupt(self):
        detector=duplex.TurnDetector()
        for _ in range(100):detector.process(np.zeros(160,dtype=np.float32),0,False)
        detector.process(np.ones(160,dtype=np.float32)*.1,1,False)
        for _ in range(100):detector.process(np.zeros(160,dtype=np.float32),0,False)
        self.assertFalse(detector.interrupted)

    def test_sustained_speech_interrupts_and_requires_complete_user_silence(self):
        detector=duplex.TurnDetector()
        with patch('faster_whisper.vad.get_speech_timestamps',return_value=[{'start':0,'end':8000}]):
            for _ in range(40):detector.process(np.ones(160,dtype=np.float32)*.03,.9,False)
        self.assertTrue(detector.interrupted);self.assertFalse(detector.finished)
        for _ in range(199):detector.process(np.zeros(160,dtype=np.float32),0,False)
        self.assertFalse(detector.finished)
        detector.process(np.zeros(160,dtype=np.float32),0,False)
        self.assertTrue(detector.finished)

    def test_short_first_word_after_long_preparation_waits_for_complete_turn(self):
        detector=duplex.TurnDetector()
        for _ in range(500):detector.process(np.zeros(160,dtype=np.float32),0,False)
        with patch('faster_whisper.vad.get_speech_timestamps',side_effect=lambda audio,options:[{'start':0,'end':len(audio)}]):
            for _ in range(12):detector.process(np.ones(160,dtype=np.float32)*.02,.1,False)
        self.assertTrue(detector.interrupted);self.assertFalse(detector.finished)
        for _ in range(199):detector.process(np.zeros(160,dtype=np.float32),0,False)
        self.assertFalse(detector.finished)
        detector.process(np.zeros(160,dtype=np.float32),0,False)
        self.assertTrue(detector.finished)

    def test_echo_adaptation_guard_starts_with_playback_not_model_preparation(self):
        detector=duplex.TurnDetector()
        for _ in range(500):detector.process(np.zeros(160,dtype=np.float32),0,False)
        for _ in range(30):detector.process(np.ones(160,dtype=np.float32)*.02,.9,True)
        self.assertFalse(detector.interrupted)

    def test_callback_stops_output_and_preserves_user_capture_without_reopening_microphone(self):
        playback=duplex.DuplexPlayback(Mock());playback.output.append((np.ones(1000,dtype=np.float32)*.2,True))
        out=np.zeros((480,1),dtype=np.float32);mic=np.ones((480,1),dtype=np.float32)*.02
        timing=SimpleNamespace(outputBufferDacTime=1.05,inputBufferAdcTime=1)
        playback._callback(mic,out,480,timing,None);self.assertTrue(np.any(out))
        playback.interrupted.set();playback._callback(mic,out,480,timing,None);self.assertFalse(np.any(out))
        self.assertEqual(playback.frames.qsize(),2)

    def test_interruption_transcript_is_used_and_can_change_language(self):
        stream=Mock();stream.__enter__=Mock(return_value=stream);stream.__exit__=Mock()
        stream.wait_until.side_effect=duplex.BargeIn();stream.speech_samples=4000
        pcm=np.ones(16000,dtype=np.float32)*.1;stream.interruption_audio.return_value=pcm
        voice=Mock();voice.synthesize.return_value=[SimpleNamespace(audio_float_array=np.ones(100,dtype=np.float32),sample_rate=24000)]
        model=Mock();model.transcribe.return_value=([SimpleNamespace(text='مرحبا',no_speech_prob=0)],SimpleNamespace(language='ar'))
        with patch.object(duplex,'DuplexPlayback',return_value=stream):
            result=transcribe.speak(Mock(),{'en':voice},'Some reply.','en',model=model,barge_in=True)
        self.assertTrue(result['interrupted']);self.assertFalse(result['spoken']);self.assertEqual(result['transcript']['text'],'مرحبا')
        self.assertIsNone(model.transcribe.call_args.kwargs['language']);self.assertFalse(model.transcribe.call_args.kwargs['vad_filter'])
        np.testing.assert_equal(model.transcribe.call_args.args[0],pcm)

    def test_output_underrun_does_not_abort_speech_but_lost_input_is_reported(self):
        playback=duplex.DuplexPlayback(Mock())
        out=np.zeros((480,1),dtype=np.float32);mic=out.copy()
        timing=SimpleNamespace(outputBufferDacTime=1.05,inputBufferAdcTime=1)
        playback._callback(mic,out,480,timing,SimpleNamespace(input_overflow=False))
        playback.check()
        playback._callback(mic,out,480,timing,SimpleNamespace(input_overflow=True))
        with self.assertRaisesRegex(RuntimeError,'Microphone audio overflowed'):playback.check()

if __name__=='__main__':unittest.main()
