# Intel macOS builds in the dedicated inventory repository

This Mac build copy publishes only to `mhddd171171-byte/Inventory`. The original
Windows project and its repository remain unchanged. The workflow
`.github/workflows/macos-intel.yml` runs on `macos-15-intel`, checks the host
architecture, builds native Intel llama.cpp and speech binaries, and packages
the same application and all offline models into a DMG.

Model weights are not stored as oversized Git objects. Build-time provisioning
uses immutable model revisions and checked SHA-256 hashes; the resulting app
contains the files and performs no model downloads at startup. Source, manifests,
scripts, tests, icons, reports/fonts and dependency notices are in Git.

The `Inventory-Manager-Intel-macOS` Actions artifact contains the `.dmg`, checksum
and Mac test evidence. GitHub Release assets must be below 2 GiB; the complete
offline installer may exceed that, so the workflow uses an Actions artifact.
Download/extract that artifact and upload the **single `.dmg`** to Google Drive
for customer distribution. Actions artifact downloads require a GitHub sign-in. Verified releases also
provide numbered DMG parts, which the project owner reassembles into one DMG. The customer should not be given development files.

## Signing

Without Apple signing credentials, the workflow builds an ad-hoc signed app.
It must not be described as Apple-notarized; macOS may require explicit approval
in Privacy & Security after a browser download. Do not disable Gatekeeper.

For Developer ID signing and notarization, configure repository Actions secrets:
`APPLE_CERTIFICATE` (base64 PKCS#12), `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), and
`APPLE_TEAM_ID`. Secrets are optional for building; never commit their values.
The workflow imports the certificate only in the temporary runner keychain.

## Verification

The build runs Python speech regressions, real Arabic/English synthesis without
audio hardware, frontend/report tests, Rust database/conversation tests, and
real local-model conversation acceptance on an isolated test SQLite database.
DMG verification mounts it read-only, checks embedded model hashes and Intel
native executables, verifies signatures, and initializes/shuts down the packaged
voice worker directly from the mounted app.

These checks do not certify the customer's RAM/performance or physical
microphone/speaker behavior. Confirm their macOS version (candidate minimum 13)
and RAM and test the final installation on that machine.

Sources: [GitHub runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[release asset limits](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases),
[Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).

## Baseline evidence portability

`tests/fixtures/macos-native-baseline` preserves the original 68 native model
responses and source provenance. Its archive SHA-256 was checked against the
original GitHub Actions artifact metadata before copying. The review script
checks the evidence checksum and guards the planner, database and models against
the evaluated Git commit. This historical evidence does not replace the new
native AI control, installed launch, persistent off, timeout and DMG tests.
No access to the original account is needed during the new repository's build.
