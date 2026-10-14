"""Evaluate real local AEC with synthetic speaker echo and overlapping speech.
No microphone, cloud or customer audio. Human/device acceptance is separate.
"""
import json
import sys
import wave
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'voice'))
from pywebrtc_audio import AudioProcessor,VoiceDetector
from duplex import TurnDetector

def read(path):
    with wave.open(str(path)) as wav:
        pcm=np.frombuffer(wav.readframes(wav.getnframes()),dtype='<i2').astype(np.float32)/32768
        rate=wav.getframerate()
    return np.interp(np.arange(0,len(pcm),rate/16000),np.arange(len(pcm)),pcm).astype(np.float32)

def main():
    source=read(ROOT/'deliverables/voice-samples/en-conversation.wav')
    user=read(ROOT/'deliverables/voice-samples/ar-conversation.wav')[:6*16000]
    far=np.pad(source,(0,4*16000));delay=960;echo=np.pad(far,(delay,0))[:len(far)]*.4
    rows=[]
    for name,with_user in [('echo-only',False),('double-talk',True)]:
        near=echo+np.random.default_rng(10).normal(0,.0003,len(far)).astype(np.float32)
        onset=4*16000
        if with_user:near[onset:onset+len(user)]+=user*.8
        ap=AudioProcessor(sample_rate=16000,echo_cancellation=True,noise_suppression=False,high_pass_filter=True,stream_delay_ms=60)
        vad=VoiceDetector(sample_rate=16000)
        detector=TurnDetector();trigger=None;cleaned=[]
        for i in range(0,len(far)-160,160):
            clean=ap.process(near[i:i+160],far[i:i+160]);cleaned.append(clean)
            if detector.process(clean,vad.process(clean),float(np.sqrt(np.mean(far[i:i+160]**2)))>.002) and trigger is None:trigger=(i+160)/16000
        pcm=np.concatenate(cleaned)
        row={'case':name,'interrupted_at_seconds':trigger,'user_onset_seconds':4 if with_user else None,'echo_rms_before':float(np.sqrt(np.mean(near[16000:onset]**2))),'echo_rms_after':float(np.sqrt(np.mean(pcm[16000:onset]**2)))}
        rows.append(row);print(json.dumps(row),flush=True)
        if with_user:
            assert trigger is not None and 4<=trigger<4.8,'Interruption did not trigger promptly.'
            capture=pcm[max(0,int(trigger*16000)-12800):int(11.5*16000)]
            from faster_whisper import WhisperModel
            model=WhisperModel(str(ROOT/'src-tauri/resources/voice/stt'),device='cpu',compute_type='int8',local_files_only=True,cpu_threads=4)
            segments,info=model.transcribe(capture,language='ar',beam_size=5,vad_filter=False)
            row['transcript']=' '.join(s.text.strip() for s in segments)
            assert row['transcript'] and ('شاشة' in row['transcript'] or 'سامسونج' in row['transcript'])
        else:assert trigger is None,'The assistant interrupted itself on its own speaker echo.'
    (ROOT/'tmp/barge-in-evaluation.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')

if __name__=='__main__':main()
