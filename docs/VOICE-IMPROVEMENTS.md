# Voice improvements — 2.0.1 and 2.0.2

## 2.0.2 conversation and interruption

The customer reports that 2.0.1 speech is much clearer. Those Nabra/Kokoro/CATT models are retained. The update adds brief grounded replies and friendly social turns through the evaluated local Qwen model, with full text under More details.

During playback a single 48 kHz full-duplex stream keeps the microphone open. WebRTC echo cancellation processes a 16 kHz capture/reference stream locally; sustained clean speech stops queued output and retains an 800 ms pre-roll. The same stream captures the rest of the question, waits two seconds of silence and transcribes locally. Input language can switch independently of the previous reply. End Conversation still cancels immediately. No application audio recordings or network services are introduced.

An initial MME-device test caught buffer overflow during packaged inference. The Windows worker now uses the user's default WASAPI input/output devices when available; it does not change OS settings. Output underruns insert silence instead of aborting a complete reply; actual microphone overflow is reported rather than fabricating a transcript.

Twenty-three Python regressions pass. Real WebRTC/Whisper simulation with an independent synthetic user voice interrupted the assistant about 137 ms after speech onset, silenced subsequent output, kept one stream and transcribed “Hi. Can you check our stock of screens?”. Echo-only evaluation caused no self-interruption. These signal tests do not establish human room/device acceptance. The final packaged worker completed four alternating English/Arabic replies on the default Windows devices in 42.34 seconds, with all sample acknowledgments and clean shutdown. Development and installed Voice Mode reached Listening with WASAPI shared-mode sample conversion, and End released their workers. The installed resources completed the four-reply hardware test in 42.63 seconds.

The prior 2.0.1 improvements and model comparison follow for provenance.

This update responds to the customer's report that speech, particularly Arabic, was unintelligible and replies stopped before their sentences finished. Inventory/accounting/database logic and the grounded Qwen assistant are unchanged.

## Faults corrected

- Redirected Python stdin used Windows ANSI (cp1252 on the development machine), while Rust sent UTF-8 JSON. Raw Arabic text could therefore reach synthesis as mojibake. The worker explicitly configures UTF-8; the supervisor also sets UTF-8 environment variables.
- Arabic and English separately initialized process-global eSpeak tables. Sequential candidate testing reproduced invalid phoneme instructions. Both production voices now use one initialization with a lock around phonemization.
- The Arabic model was a 7.48M distilled model. The replacement is the full Nabra-82M FP32 model with local CATT-EO int8 vowel restoration, app-vocabulary pronunciation fixes and natural sentence endings. English uses Kokoro-82M q8, af_heart.
- Speech restarted the output device for each chunk and switched sample rates between voices. It now uses one 24 kHz stream, prepares the next phrase during playback, drains queued audio and retains a final silence/echo tail before listening resumes.
- A fixed 150-second Rust response timeout could kill long replies mid-sentence. Speech now has a text-length budget and progress acknowledgements; only final completion resumes listening. Explicit End Conversation and closing the app still cancel the owned process promptly.
- Sentence splitting damaged decimal prices. Amounts are now spoken as exact rials/baisa, without changing chat or database values. Phone/model codes are retained. Phoneme splits preserve whole words instead of slicing at arbitrary character positions.
- Listening ended after about 900 ms silence and forcibly ended speech after 20 seconds. It now allows two seconds of silence and up to 90 seconds of continuous speech. Reaching the safety limit reports the need to pause instead of treating an incomplete question as complete.

## Model comparison

Actual CPU inference compared the previous voices with full Nabra-82M, Nabra with CATT, and English Kokoro FP32/q8 using four inventory/help phrases per candidate (24 recordings in total). English q8 retained the tested sentences and generated speech faster than playback, while saving 233 MB compared with FP32. Arabic CATT improved vowel restoration for the app guidance sample; proper names still needed the supplied pronunciation corrections. Machine transcription is an imperfect intelligibility diagnostic, not human listening acceptance.

Pinned weights: Arabic 325,639,514 bytes; English 92,361,116 bytes; CATT 21,564,456 bytes. Selected synthesis took roughly 1–2 seconds for 6–10 seconds of speech on the Windows development machine (Intel i9-13900HX). These timings do not establish older Intel i5 Mac performance. All synthesis runs on CPU, with at most four threads. Models/voice packs/tokenizer tables are local and included in the offline installer; there are no startup downloads or hosted voice APIs.

Arabic model: [Nabra-82M](https://huggingface.co/oddadmix/Nabra-82M-v0.1), [FP32 ONNX](https://huggingface.co/marwanelamami/Nabra-82M-v0.1-ONNX). English: [Kokoro-82M ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX). Vowel restoration: [CATT-EO](https://huggingface.co/TigreGotico/catt-diacritizer). Apache-2.0 notices, upstream model cards and CATT tokenizer sources are bundled; Piper/eSpeak GPL sources remain supplied.

## Verification

The automated regressions cover final audio samples and draining, one output stream, multiple phrases/languages, long phoneme preservation, natural pauses, speech beyond the former 20-second limit, currency precision/negative values, original Arabic letters/numbers, and one-time engine initialization. Run `.build/voice-env/Scripts/python.exe -m unittest discover -s voice -p 'test_*.py'`.

The opt-in native test sends real UTF-8 Arabic/English through the Rust supervisor and packaged executable, plays it on the default speaker, verifies complete progress/final acknowledgements and stops only its child. Run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/desktop.ps1 test-voice`. Set `INVENTORY_LONG_VOICE_TEST=1` for a spoken reply beyond 150 seconds; this audibly plays several minutes of test speech. `INVENTORY_VOICE_TEST_RESOURCES` can point to an installed application's `resources/voice` directory.

Windows results: 15 voice regression tests passed; the native Rust supervisor played four alternating bilingual replies with preserved input-character counts. Its long reply contained 2,874 characters and 4,426,800 speech samples, and completed in 186.42 seconds, beyond the previous cutoff. Clean worker shutdown passed. The installed copy passed the same four bilingual replies (44.17 seconds total); both development and installed interfaces reached Listening automatically after Start, and End removed their owned workers. The existing 14 frontend tests, 36 backend tests and 12 browser fixture flows passed; browser voice sequencing uses mocked IPC, whereas the native playback test uses real models/output hardware. ONNX Runtime 1.23.2 separately loaded both voices and CATT and synthesized valid audio on Windows.

The customer has reported substantially improved clarity with these voices. The included WAV samples preserve their evaluation examples. Arabic is Modern Standard Arabic with a fixed synthetic voice. Repeated human interruptions under the customer's room/device conditions remain an acceptance check. No physical Intel Mac test is available yet.
