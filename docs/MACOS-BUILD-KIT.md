# Inventory Manager 2.0.3 — Intel Mac build handoff

**This ZIP is a developer build kit, not a customer installer. No native macOS
build or installed Mac test has been completed. Do not send it to the customer
as an installer.**

It contains the existing application source, all checksum-verified shared
model files, their notices, and llama.cpp b11554 source. Windows executables,
Windows DLLs, customer databases and signing credentials are excluded.

## Build computer

Use Intel macOS 13+ with Xcode Command Line Tools, Python 3.12, Node.js, Rust and
CMake. Customer macOS/RAM still must be confirmed; this minimum is a candidate
dependency profile, not a hardware performance guarantee. Build-time tool and
Python dependency provisioning can use the internet. The installed application
must not download models or runtimes.

Extract the ZIP, open Terminal in its extracted folder, and run:

```sh
bash scripts/build-macos-intel.sh
```

The script verifies all models, extracts the included llama.cpp source, builds
native Intel inference and voice helpers, stages models/resources and Mac
dependency notices, runs automated tests, and requests Tauri `.app` and `.dmg`
bundles. The output is under
`src-tauri/target/x86_64-apple-darwin/release/bundle/`.

There is no Python/Node/Rust installation requirement on the customer's computer;
the final app contains the native voice runtime and model weights.

## Customer release

Configure the Developer ID signing identity and notarization credentials locally
on the Mac using Tauri's documented environment variables. Never put credentials
in this ZIP or source files. All nested native helpers/libraries need consistent
signing. When `APPLE_SIGNING_IDENTITY` is set, the build script passes that
identity to PyInstaller for its nested native binaries and signs the native
llama-server before Tauri packages the application. Verify codesign,
notarization/stapling and Gatekeeper on the final DMG;
the current script does not certify or automatically approve a customer release.

Test an installation from the DMG in Applications, fully disconnected: all
inventory/search/report flows, automatic AI startup/recovery/shutdown, bilingual
chat, microphone permission/denial/recovery, complete voice replies, repeated
speaker barge-in, and operation after moving the app out of its build directory.
Measure RAM and response time on the customer's Intel Mac. Do not claim the
Windows test results certify Mac operation or reduce model size without rerunning
the conversation/voice acceptance checks.

Only after these checks pass, upload the resulting **Intel `.dmg`** to Google
Drive. The customer downloads it, opens it, drags Inventory Manager to
Applications, and launches Inventory Manager from Applications. They do not
receive this build kit or run development commands.

References: [Tauri DMG packaging](https://v2.tauri.app/distribute/dmg/) and
[Tauri macOS signing/notarization](https://v2.tauri.app/distribute/sign/macos/).
