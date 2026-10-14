# Offline AI and speech — licenses and operation

Selected resources were reviewed and tested during Windows development on 14 October 2026. All model/inference/speech packages below are local; no paid or hosted inference API is used. Windows platform components have separate Microsoft terms.

## Model and runtime

| Component | Artifact | License |
|---|---|---|
| llama.cpp | Windows x64 CPU release b11554, native CPU variants and DLLs | MIT; full notices beside binaries |
| Qwen3 | Qwen3-4B-Instruct-2507 Q4_K_M, bartowski GGUF quantization of the official Qwen model | Apache-2.0 |
| LLVM OpenMP | libomp supplied in the llama.cpp release | Apache-2.0 with LLVM exceptions |

Original model: [Qwen3-4B-Instruct-2507](https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507). Its Apache-2.0 license and model card accompany the runtime. The pinned quantization is [bartowski GGUF revision ae44f08e1392f39c0e474af10c3ff8355c8b6688](https://huggingface.co/bartowski/Qwen_Qwen3-4B-Instruct-2507-GGUF/tree/ae44f08e1392f39c0e474af10c3ff8355c8b6688).

The bundled GGUF is 2,497,280,736 bytes, SHA-256 `2fde00ce69dd4899c70d020845e2638353015bba0fdf161b3eb965f2bca4464e`. Provenance is preserved in `resources/ai/MODEL-SOURCE.json`; per-file hashes are in `SHA256SUMS.txt`. Quantization changes numerical precision, not business facts. The evaluated 0.6B and 1.7B models are retained outside the installer and are not loaded on customer startup.

Downloads were authorized by the user. Model choice is based on real bilingual/multi-turn SQLite evaluations. With the same final planner, 1.7B passed 33/41 checks and 4B passed 41/41. Development-machine results and hardware limits are in VALIDATION.md. The production profile uses non-thinking CPU inference, 4096 context tokens and at most four threads. A 2048-token low-memory profile is supported; Intel i5 Mac latency and combined voice memory still require physical testing.

## Packaged speech

| Component | Purpose and selected model | License / source |
|---|---|---|
| faster-whisper / CTranslate2 | Multilingual Whisper small, CPU int8 | MIT; Systran/faster-whisper-small revision 536b0662742c02347bc0e980a01041f333bce120 |
| Silero VAD | Bundled faster-whisper voice activity graph | MIT notices in worker packages |
| pywebrtc-audio 0.2.0 | Local full-duplex echo cancellation and spectral speech detection | Apache-2.0 binding; upstream WebRTC/third-party notices bundled with the wheel |
| Nabra-82M FP32 ONNX | Arabic MSA speech, af_msa voice | Apache-2.0; marwanelamami/Nabra-82M-v0.1-ONNX revision 85065b0be8573aefb401f8f53e7edc37d6556186, from oddadmix/Nabra-82M-v0.1 |
| Kokoro-82M q8 ONNX | English US speech, af_heart voice | Apache-2.0; onnx-community/Kokoro-82M-v1.0-ONNX revision 1939ad2a8e416c0acfeecc08a694d14ef25f2231, from hexgrad/Kokoro-82M |
| CATT-EO int8 ONNX | Arabic short-vowel restoration | Apache-2.0; TigreGotico/catt-diacritizer revision 8ad763613cce96511c099e9943820be7cc80fbc2; original model/tokenizer © Abjad AI |
| Piper 1.8 / eSpeak NG | Local phonemization | GPL-3.0; complete corresponding upstream source archives included |
| libtashkeel | Piper's bundled alternative diacritizer (production voice uses CATT) | MIT |
| ONNX Runtime / NumPy / sounddevice / PortAudio | Local inference and audio | MIT / BSD notices supplied by exact wheels |
| Python / PyInstaller | Self-contained worker runtime | Python license; PyInstaller GPL with bootloader exception |

`resources/voice/MODELS.json` pins every speech artifact URL, revision, size and hash. Full dependency metadata, notices, GPL sources, the worker's GPL source, the Arabic adapter source, the exact build requirements and the faster-whisper PCM-only decoding patch are in `resources/licenses/voice/`. The worker runs as a separate local process communicating by bounded JSON; it is not a remote service. Preserve all corresponding source and notices with redistributed copies. The installer includes these materials.

Voice has real spoken output in both languages. Users need no Python installation or manual speech setup. Models load only on Start Conversation. Microphone audio is held in memory and is not uploaded or written to disk. End Conversation cancels only the owned worker. Bilingual product names are spoken using the appropriate local voice.

The [Nabra model card](https://huggingface.co/oddadmix/Nabra-82M-v0.1), [ONNX export](https://huggingface.co/marwanelamami/Nabra-82M-v0.1-ONNX), [Kokoro model card](https://huggingface.co/hexgrad/Kokoro-82M), [English ONNX export](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX), and [CATT export](https://huggingface.co/TigreGotico/catt-diacritizer) describe the models and their Apache-2.0 terms. Copies and CATT's original tokenizer/helper sources and license are bundled. Arabic targets Modern Standard Arabic; dialectal or perfectly human-quality speech is not guaranteed. Measured comparisons and listening samples are described in VOICE-IMPROVEMENTS.md.

## Offline and database boundary

The llama.cpp server binds only to a randomly allocated keyed loopback port. Its client disables proxies and redirects; inherited network/model override variables are removed. Startup uses --offline, no web UI, no remote model URL, no multimodal projector or agent tools.

The LLM receives the current message plus bounded conversation context and emits a schema-constrained semantic plan. Rust validates operations, parameters and references, resolves ambiguity and computes facts from fresh approved SQLite results. The model cannot open SQLite, execute SQL, launch commands, or write inventory. Scope clarification is semantic, not a rigid keyword intent classifier. Approximate product matches are labeled as suggestions before their figures can be used. Restore clears context; each conversation has a separate identifier.

The speech worker uses local-files-only loading plus offline environment flags. A Python audit hook rejects network connection/address-resolution/bind operations. Runtime model downloads, cloud APIs, telemetry and automatic application updates are absent.

## Microsoft platform exceptions

The official Windows runtime imports Microsoft C++ DLLs. Unmodified app-local versions 14.50.35719.0 accompany llama.cpp; their signatures/provenance were checked. Windows supplies OS/UCRT support. WebView2's offline installer is embedded in the Windows package. These dependencies are proprietary platform components and are not covered by the application's open-source library licenses.

Microsoft redistribution rights and runtime terms are documented in PLATFORM-RUNTIMES.md and preserved notices. Do not describe the complete Windows distribution as entirely open source or apply an MIT/Apache/GPL license to Microsoft's binaries.
