"""Opt-in device diagnostic. Counts buffer flags; does not retain audio."""
import collections
import time
import sounddevice as sd
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'voice'))
from transcribe import load_voices
voices=load_voices(Path(__file__).resolve().parents[1]/'src-tauri/resources/voice')

for latency in ('low',(0.1,0.1),'high'):
    counts=collections.Counter()
    def callback(mic,out,frames,timing,status):
        out.fill(0)
        if status: counts[str(status)]+=1
    with sd.Stream(samplerate=48000,channels=(1,1),dtype='float32',blocksize=480,latency=latency,callback=callback) as stream:
        for lang,text in [('en','You can search by product name or device model.'),('ar','مرحبا كيف يمكنني مساعدتك في المخزون اليوم؟')]:
            list(voices[lang].synthesize(text))
        time.sleep(1)
        print(latency,stream.latency,dict(counts),flush=True)
