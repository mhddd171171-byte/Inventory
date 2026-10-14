# Packaged offline two-way voice

Voice Mode is integrated into AI Assistant. Start Conversation automatically loads the supplied local models; End Conversation cancels capture/playback and releases the owned worker. Users do not install Python or configure executable/model paths.

The worker uses local VAD, CPU-int8 multilingual faster-whisper-small, English LJSpeech Piper TTS and Arabic Nabra MSA TTS. Audio stays in memory. Microphone capture ends before inference/playback and resumes after playback completes, avoiding self-hearing. Utterances and protocol requests are bounded. Offline environment flags and a Python audit hook prohibit worker network access.

Build-time only:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-voice.ps1
.build/voice-env/Scripts/python.exe -m unittest discover -s voice -p test_transcribe.py
```

`requirements-lock.txt` records the exact Windows developer environment. Encoded-media AV decoding is excluded from the packaged worker because capture passes PCM arrays directly. The small compatibility patch and complete worker source are distributed with the runtime notices. `evaluate.py` tests synthesized English/Arabic utterances through actual STT/TTS models; this does not substitute for microphone/speaker testing with a human.

Voice may be slower on older Intel CPUs and depends on local audio hardware and microphone permission. Model quality and native Intel Mac compatibility must be confirmed on the target computer. See `docs/AI-LICENSES.md` and `docs/MACOS-PREPARATION.md`.
