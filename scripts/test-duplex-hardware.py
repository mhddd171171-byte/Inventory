"""Opt-in local playback/duplex diagnostic. No microphone recordings saved."""
from pathlib import Path
import sys
import json
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'voice'))
from transcribe import load_model,load_voices,speak
from faster_whisper.vad import get_vad_model
resources=Path(__file__).resolve().parents[1]/'src-tauri/resources/voice'
sd,model=load_model(resources/'stt');voices=load_voices(resources);get_vad_model()
if '--wasapi' in sys.argv:
    api=next(a for a in sd.query_hostapis() if a['name']=='Windows WASAPI')
    sd.default.device=(api['default_input_device'],api['default_output_device'])
print('Devices',sd.default.device,flush=True)
print(json.dumps(speak(sd,voices,'You can search by product name or device model.','en',model=model,barge_in=True),ensure_ascii=True),flush=True)
