# Intel macOS preparation — not yet verified

The customer's MacBook is an Intel Core i5. Its model/year, RAM and macOS version are unknown. No Mac build, VM run or physical Mac performance test has been performed in this Windows workspace. Windows development continues independently.

The shared React, Rust, SQLite, report, conversation and voice protocol code uses platform-neutral paths. Only the packaged native runtimes differ. `scripts/build-macos-intel.sh` prepares an Intel `.app` and `.dmg` on a suitable Mac; it refuses to run on Windows or ARM. It stages a native CPU llama.cpp build and a native PyInstaller voice worker alongside the same GGUF/ONNX/CTranslate2 model files. No Windows DLLs or executables enter the Mac resource mapping.

The candidate dependency profile targets **macOS 13 or later** because ONNX Runtime 1.23.2 publishes a CPython 3.12 Intel wheel for that version. The Windows build uses ONNX Runtime 1.31.0, which does not publish an Intel macOS wheel. This is a build prerequisite, not confirmation that the customer's Mac can run it. An older macOS version requires a separately validated dependency profile or native dependency build before deployment. Do not silently remove voice or assume an OS upgrade is possible.

Build-time tools: Xcode Command Line Tools, Rust stable, Node.js, CMake and Python 3.12. Set `LLAMA_SOURCE` to a reviewed llama.cpp b11554 source checkout, retain the supplied model resources, and run:

```sh
bash scripts/build-macos-intel.sh
```

Python and development tools are needed only on the build computer. The installed app includes its own voice runtime. Build-time dependency provisioning may require internet; normal application startup does not.

`Info.plist` explains local microphone use; `Entitlements.plist` declares audio input for signed builds. Sign nested helper executables/libraries and the main app consistently before notarization. Verify the helper inherits the app's microphone consent on the actual signed package; no permission approval is bypassed automatically.

The Mac resource profile starts with 2,048 context tokens and four CPU threads. Benchmark initialization, peak combined memory, Arabic/English accuracy, multi-turn latency and voice latency on the customer's physical Mac. Measure responsiveness while both models are resident. Choose a smaller model only after rerunning acceptance tests. The worker releases its models when End Conversation is pressed.

Voice version 2.0.1 uses portable CPU ONNX Nabra-82M FP32, Kokoro-82M q8 and CATT-EO int8. Both voices loaded and synthesized successfully with ONNX Runtime 1.23.2 on Windows, which verifies compatibility with that runtime version's model interface/kernels. It does not verify the Intel macOS wheel, microphone/speaker drivers, native worker packaging or Mac performance. Keep sentence/word chunks bounded; do not replace Arabic FP32 with unmeasured quantization merely to reduce file size.

Version 2.0.2 retains those models and adds pywebrtc-audio 0.2.0 for local echo cancellation. Its published CPython 3.12 macOS x86_64 wheel targets macOS 10.13+, but neither that wheel nor duplex CoreAudio playback has been tested on a physical Intel Mac. The Mac requirements pin the dependency; native packaging must include its extension and all notices. Combined model memory, barge-in latency and echo behavior must be measured on the customer's hardware before deployment.

Before release, verify installed launch, automatic model discovery, recovery after a helper crash, clean process exit, every inventory/report operation, microphone denial/recovery, real speaker playback, repeated bilingual voice turns and fully disconnected operation. A VM can provide preliminary functionality checks; it cannot establish physical Mac performance.

Sources: [Tauri platform configuration](https://v2.tauri.app/reference/config/), [ONNX Runtime 1.23.2 distributions](https://pypi.org/project/onnxruntime/1.23.2/#files), [Piper distributions](https://pypi.org/project/piper-tts/1.8.0/#files).
