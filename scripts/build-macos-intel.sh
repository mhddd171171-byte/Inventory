#!/usr/bin/env bash
# Preparation workflow: run on an Intel Mac with Xcode CLI tools, Rust, Node,
# CMake and Python 3.12. This script is never run by the installed application.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != x86_64 ]]; then
  echo 'Build on Intel macOS. Windows binaries cannot be repackaged as Mac binaries.' >&2
  exit 1
fi
for tool in python3.12 node npm cargo rustup cmake xcrun; do
  command -v "$tool" >/dev/null || { echo "Missing build tool: $tool" >&2; exit 1; }
done
python3.12 scripts/verify-macos-models.py
npm ci
npm test
npm run build
if [[ -z "${LLAMA_SOURCE:-}" && -f build-inputs/llama-b11554.tar.gz ]]; then
  python3.12 scripts/extract-macos-llama.py
  LLAMA_SOURCE="$PWD/.build/llama-source/llama.cpp-b11554"
fi
: "${LLAMA_SOURCE:?Set LLAMA_SOURCE to the reviewed llama.cpp b11554 source checkout}"
[[ -f "$LLAMA_SOURCE/CMakeLists.txt" ]] || { echo 'Invalid llama.cpp source directory.' >&2; exit 1; }
export MACOSX_DEPLOYMENT_TARGET=13.0
python3.12 -m venv .build/voice-macos-env
.build/voice-macos-env/bin/python -m pip install -r voice/requirements-macos-intel.txt
.build/voice-macos-env/bin/python voice/patch_faster_whisper.py
.build/voice-macos-env/bin/python -m unittest discover -s voice -p 'test_*.py'
voice_options=(--target-architecture x86_64)
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" && "$APPLE_SIGNING_IDENTITY" != '-' ]]; then
  voice_options+=(--codesign-identity "$APPLE_SIGNING_IDENTITY" --osx-entitlements-file src-tauri/Entitlements.plist)
fi
.build/voice-macos-env/bin/python -m PyInstaller --noconfirm --onedir --console \
  "${voice_options[@]}" \
  --runtime-hook scripts/macos-voice-runtime-hook.py \
  --name voice-worker --distpath .build/voice-macos-dist --workpath .build/voice-macos-work --specpath .build \
  --collect-all faster_whisper --collect-all piper --collect-all ctranslate2 \
  --collect-all onnxruntime --collect-all sounddevice --collect-all _sounddevice_data --collect-all tokenizers --collect-all pywebrtc_audio \
  --exclude-module av --exclude-module piper.train --exclude-module piper.http_server voice/transcribe.py
if [[ -x .build/llama-macos/bin/llama-server ]] && [[ -f .build/llama-macos/CMakeCache.txt ]] && \
  grep -q 'GGML_NATIVE:BOOL=OFF' .build/llama-macos/CMakeCache.txt && \
  grep -q 'GGML_METAL:BOOL=OFF' .build/llama-macos/CMakeCache.txt && \
  [[ "$(lipo -archs .build/llama-macos/bin/llama-server)" == x86_64 ]]; then
  echo 'Reusing cached b11554 Intel CPU runtime from the checksum-pinned native cache.'
else
cmake -S "$LLAMA_SOURCE" -B .build/llama-macos \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES=x86_64 \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=13.0 -DGGML_METAL=OFF -DGGML_NATIVE=OFF -DBUILD_SHARED_LIBS=OFF \
  -DLLAMA_CURL=OFF -DLLAMA_BUILD_TESTS=OFF -DGGML_OPENMP=OFF
cmake --build .build/llama-macos --config Release --target llama-server -j 4
fi
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" && "$APPLE_SIGNING_IDENTITY" != '-' ]]; then
  codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" .build/llama-macos/bin/llama-server
fi
.build/voice-macos-env/bin/python -c 'import platform; assert platform.machine() == "x86_64"'
.build/voice-macos-env/bin/python scripts/stage-macos-resources.py
.build/voice-macos-env/bin/python scripts/collect-voice-licenses.py --output .build/macos-resources/licenses/voice
.build/voice-macos-env/bin/python scripts/smoke-macos-speech.py
printf '{"shutdown":true}\n' | .build/macos-resources/voice/runtime/voice-worker --worker --resources .build/macos-resources/voice
rustup target add x86_64-apple-darwin
cargo test --release --target x86_64-apple-darwin --manifest-path src-tauri/Cargo.toml
# Test the same profile before packaging, while Cargo's test binary is current.
INVENTORY_AI_RESOURCE_DIR="$PWD/.build/macos-resources/ai" \
INVENTORY_AI_EVAL_OUTPUT="$PWD/.build/MACOS-AI-ACCEPTANCE.json" \
VECLIB_MAXIMUM_THREADS=1 \
cargo test --release --target x86_64-apple-darwin --manifest-path src-tauri/Cargo.toml independent_assistant_acceptance -- --ignored --nocapture
for key in APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH; do
  if [[ -z "${!key:-}" ]]; then unset "$key"; fi
done
npx tauri build --target x86_64-apple-darwin --bundles app,dmg
echo 'Candidate .app/.dmg built. Signing, microphone permission, installed launch and physical Mac testing remain required.'
