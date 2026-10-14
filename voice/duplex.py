"""Offline full-duplex playback with WebRTC echo cancellation and barge-in.

PortAudio's callback only moves audio. Echo cancellation and speech detection
run on a dedicated thread. The microphone's first words remain in a pre-roll
buffer; stopping output does not close/reopen the microphone mid-utterance.
"""
import collections
import queue
import threading
import time
import numpy as np
from pywebrtc_audio import AudioProcessor,VoiceDetector


class BargeIn(Exception):
    pass


class TurnDetector:
    """Sustained clean speech triggers interruption, not one click/noise frame."""
    def __init__(self):
        self.total = 0
        self.rendered = 0
        self.run = 0
        self.energy_run = 0
        self.recent = collections.deque(maxlen=50)
        self.noise = 0.0015
        self.interrupted = False
        self.last_speech = 0

    def process(self, clean, probability, rendering):
        self.total += len(clean)
        if rendering:self.rendered+=len(clean)
        rms = float(np.sqrt(np.mean(clean * clean)))
        self.recent.append(clean.copy())
        if not rendering and probability < 0.25:
            self.noise = 0.99 * self.noise + 0.01 * min(rms, 0.008)
        energetic = rms >= max(0.003, self.noise * 3)
        speech = probability >= 0.55 and energetic
        # Allow the echo filter to adapt before accepting speaker residuals.
        if self.rendered < 5600 and rendering:
            speech = False
            energetic = False
        self.energy_run = self.energy_run + len(clean) if energetic else max(0,self.energy_run-len(clean))
        # Spectral speech probability can rise slowly on the first short word
        # ("hi", "stop"). Confirm energetic candidates with the bundled neural
        # VAD so that a click or door slam alone does not stop the assistant.
        if not self.interrupted and self.energy_run>=1280 and self.total%1280==0:
            from faster_whisper.vad import get_speech_timestamps,VadOptions
            window=np.concatenate(self.recent)
            segments=get_speech_timestamps(window,VadOptions(threshold=0.55,min_speech_duration_ms=96,min_silence_duration_ms=100,speech_pad_ms=0))
            if segments and segments[-1]["end"]>=len(window)-1600:
                self.interrupted=True
                self.last_speech=self.total
        self.run = self.run + len(clean) if speech else max(0, self.run - len(clean))
        if not self.interrupted and self.run >= 1920:
            self.interrupted = True
        if speech:
            self.last_speech = self.total
        return self.interrupted

    @property
    def finished(self):
        return self.interrupted and self.total - self.last_speech >= 32000


class DuplexPlayback:
    def __init__(self, sd, on_interrupt=None):
        self.sd = sd
        self.on_interrupt = on_interrupt
        self.frames = queue.Queue(maxsize=300)
        self.output = collections.deque()
        self.current = None
        self.offset = 0
        self.condition = threading.Condition()
        self.interrupted = threading.Event()
        self.finished = threading.Event()
        self.closed = threading.Event()
        self.error = None
        self.source_written = 0
        self.source_played = 0
        self.speech_samples = 0
        self.pre_roll = collections.deque(maxlen=80)
        self.captured = []
        self.captured_samples = 0
        self.was_interrupted = False

    def __enter__(self):
        self.processor = AudioProcessor(sample_rate=16000, echo_cancellation=True,
            noise_suppression=False, high_pass_filter=True, auto_gain_control=False, stream_delay_ms=80)
        self.voice_detector = VoiceDetector(sample_rate=16000)
        self.detector = TurnDetector()
        self.monitor = threading.Thread(target=self._monitor, daemon=True)
        self.monitor.start()
        try:
            self.stream = self.sd.Stream(samplerate=48000, channels=(1, 1), dtype="float32",
                blocksize=480, latency=(0.1,0.1), callback=self._callback)
            self.stream.start()
        except Exception:
            self.closed.set()
            self.monitor.join(timeout=2)
            raise
        return self

    def _callback(self, microphone, speaker, frames, timing, status):
        speaker.fill(0)
        if status and status.input_overflow:
            # Lost microphone frames cannot be recovered. Output underruns
            # only insert silence; they must not discard a complete reply.
            self.error = RuntimeError(f"Microphone audio overflowed ({status}, played {self.speech_samples//2} samples). Try a working microphone/speaker pair.")
        with self.condition:
            if not self.interrupted.is_set():
                position = 0
                while position < frames:
                    if self.current is None:
                        if not self.output:
                            break
                        self.current, self.is_speech = self.output.popleft()
                        self.offset = 0
                    count = min(frames-position, len(self.current)-self.offset)
                    speaker[position:position+count, 0] = self.current[self.offset:self.offset+count]
                    self.source_played += count
                    if self.is_speech:
                        self.speech_samples += count
                    position += count
                    self.offset += count
                    if self.offset == len(self.current):
                        self.current = None
            self.condition.notify_all()
        try:
            delay = max(0, min(500, int((timing.outputBufferDacTime-timing.inputBufferAdcTime)*1000)))
            self.frames.put_nowait((microphone[:,0].copy(), speaker[:,0].copy(), delay))
        except queue.Full:
            self.error = RuntimeError("Voice processing fell behind the audio device.")

    def _monitor(self):
        try:
            while not self.closed.is_set():
                try:
                    near, far, delay = self.frames.get(timeout=0.1)
                except queue.Empty:
                    continue
                # 48 kHz output preserves the model's 24 kHz waveform. The
                # echo-cancelled capture is 16 kHz, matching Whisper's input.
                near = near.reshape(-1,3).mean(axis=1).astype(np.float32)
                far = far.reshape(-1,3).mean(axis=1).astype(np.float32)
                self.processor.stream_delay_ms = delay
                clean = self.processor.process(near, far)
                rendering = float(np.sqrt(np.mean(far*far))) > 0.002
                self.pre_roll.append(clean.copy())
                detected = self.detector.process(clean, self.voice_detector.process(clean), rendering)
                if detected and not self.was_interrupted:
                    self.was_interrupted = True
                    self.captured.extend(self.pre_roll)
                    self.captured_samples = sum(map(len,self.captured))
                    self.interrupted.set()  # next 10ms output callback is silent
                    with self.condition:
                        self.output.clear()
                        self.current = None
                        self.condition.notify_all()
                    if self.on_interrupt:
                        self.on_interrupt(self.speech_samples // 2)
                elif self.was_interrupted:
                    self.captured.append(clean.copy())
                    self.captured_samples += len(clean)
                if self.detector.finished:
                    self.finished.set()
                if self.was_interrupted and self.captured_samples >= 90*16000:
                    self.error = ValueError("Please pause after your question (up to 90 seconds per turn).")
                    self.finished.set()
        except Exception as exc:
            self.error = exc
            self.finished.set()
            self.interrupted.set()
            with self.condition:
                self.condition.notify_all()

    def check(self):
        if self.error:
            raise self.error
        if self.interrupted.is_set():
            raise BargeIn()

    def write(self, pcm, speech=False):
        self.check()
        pcm = pcm.reshape(-1)
        # Linear 2x upsampling; no high-frequency speech is discarded.
        converted = np.interp(np.arange(len(pcm)*2)/2, np.arange(len(pcm)), pcm).astype(np.float32)
        with self.condition:
            target = self.source_written + len(converted)
            self.source_written = target
            self.output.append((converted, speech))
            while self.source_played < target:
                self.check()
                self.condition.wait(timeout=0.03)

    def wait_until(self, future):
        while not future.done():
            self.check()
            time.sleep(0.01)
        self.check()
        return future.result()

    def interruption_audio(self):
        if not self.finished.wait(timeout=93):
            raise RuntimeError("Please finish your question, then try again.")
        if self.error:
            raise self.error
        return np.concatenate(self.captured).astype(np.float32)

    def __exit__(self, *args):
        self.closed.set()
        # All writes are consumed before normal close; interrupt close aborts
        # queued output immediately, then releases only these audio streams.
        self.stream.abort() if self.interrupted.is_set() else self.stream.stop()
        self.stream.close()
        self.monitor.join(timeout=2)
