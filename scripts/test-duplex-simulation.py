"""Exercise the production duplex callback/AEC/Whisper handoff without devices.
Simulates delayed speaker echo and a person speaking over a longer reply.
"""
import json
import sys
import threading
import time
import wave
from pathlib import Path
from types import SimpleNamespace
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'voice'))
import transcribe

def read(path):
    with wave.open(str(path)) as wav:
        assert wav.getframerate()==24000
        return np.frombuffer(wav.readframes(wav.getnframes()),dtype='<i2').astype(np.float32)/32768

class SimulatedStream:
    def __init__(self, owner, **options):
        self.owner=owner;self.callback=options['callback'];self.running=False;self.played=[];self.tick=0
    def start(self):
        self.running=True;self.thread=threading.Thread(target=self.loop);self.thread.start()
    def loop(self):
        echo=[np.zeros(480,dtype=np.float32) for _ in range(8)]
        user=np.repeat(self.owner.user,2);onset=2*48000;started=time.perf_counter()
        while self.running:
            i=self.tick*480
            mic=echo.pop(0)*.35
            if onset<=i<onset+len(user):mic+=user[i-onset:i-onset+480]*.9
            out=np.zeros((480,1),dtype=np.float32)
            self.callback(mic[:,None],out,480,SimpleNamespace(outputBufferDacTime=i/48000+.08,inputBufferAdcTime=i/48000),None)
            echo.append(out[:,0].copy());self.played.append(out[:,0].copy());self.tick+=1
            time.sleep(max(0,started+self.tick*.01-time.perf_counter()))
    def stop(self):self.running=False;self.thread.join()
    def abort(self):self.stop()
    def close(self):pass

class SimulatedAudio:
    def __init__(self,user):self.user=user;self.streams=[]
    def Stream(self,**options):
        stream=SimulatedStream(self,**options);self.streams.append(stream);return stream

def main():
    audio=read(ROOT/'deliverables/voice-samples/en-conversation.wav')
    source=SimpleNamespace(synthesize=lambda text:[SimpleNamespace(audio_float_array=audio,sample_rate=24000)])
    # The simulated person must be an independent voice/phrase. Replaying the
    # assistant's identical PCM is correctly identified as speaker echo.
    from piper import PiperVoice
    person=PiperVoice.load(str(ROOT/'.build/voice-model-evaluation/previous/tts/en_US-ljspeech-high.onnx'))
    spoken=list(person.synthesize('Hi, can you check our stock of screens?'))
    user_pcm=np.concatenate([c.audio_float_array for c in spoken])
    user_pcm=np.interp(np.arange(0,len(user_pcm),spoken[0].sample_rate/24000),np.arange(len(user_pcm)),user_pcm).astype(np.float32)
    user_pcm=np.pad(user_pcm,(0,(-len(user_pcm))%240))
    sd=SimulatedAudio(user_pcm);events=[]
    from faster_whisper import WhisperModel
    model=WhisperModel(str(ROOT/'src-tauri/resources/voice/stt'),device='cpu',compute_type='int8',local_files_only=True,cpu_threads=4)
    started=time.perf_counter()
    class RecordingModel:
        def transcribe(self,pcm,**kwargs):
            self.pcm=pcm.copy()
            return model.transcribe(pcm,**kwargs)
    recorder=RecordingModel()
    result=transcribe.speak(sd,{'en':source},'This is a longer assistant reply.','en',model=recorder,barge_in=True,on_interrupt=lambda samples:events.append((time.perf_counter()-started,samples)))
    print(json.dumps({'result':result,'events':events}),flush=True)
    with wave.open(str(ROOT/'tmp/duplex-captured.wav'),'wb') as wav:
        wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(16000)
        wav.writeframes((np.clip(recorder.pcm,-1,1)*32767).astype('<i2').tobytes())
    assert result['interrupted'] and not result['spoken']
    assert len(sd.streams)==1 and events
    assert events[0][0]<3.2,'Speaker did not stop promptly after user speech.'
    assert 'hi' in result['transcript']['text'].lower(),'The first interruption word was lost.'
    speaker=np.concatenate(sd.streams[0].played)
    assert not np.any(speaker[int((events[0][0]+.08)*48000):]),'Speaker resumed the cancelled reply.'
    result.update(interruption_seconds=events[0][0],user_audio_onset_seconds=2,stream_count=1,initial_word_preserved=True)
    (ROOT/'tmp/duplex-simulation.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result),flush=True)

if __name__=='__main__':main()
