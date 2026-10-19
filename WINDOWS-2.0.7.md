# Windows 2.0.7

Windows counterpart of the delivered Intel Mac 2.0.7 release (Mac base 17d1039).

Uses the same inventory, CR search, reports, out-of-stock view, chat and SQLite
schema. Enables the Mac release's Settings, appearance, saved AI on/off state,
20-minute startup deadline with explicit retry, and automatic versioned backups
on Windows too. Opening the backup folder uses Windows Explorer. The existing
application identifier and Windows installation/data paths remain unchanged.

Mac installation-only safeguards remain conditional on macOS. AI and voice use
Windows native runtimes with the existing bundled model files. No customer data
is embedded. Windows 2.0.4 and the delivered Mac installer are preserved.

Build: provision the existing Windows offline resources under src-tauri/resources,
install project npm dependencies, then run scripts/desktop.ps1 build. The Inno
compiler and offline WebView2 installer must be present as for Windows 2.0.4.
CARGO_TARGET_DIR is supported for the application executable during packaging.
Output: deliverables/Inventory Manager_2.0.7_x64-setup.exe.

Validation: 52 backend tests, 21 frontend tests, 20 browser IPC integration checks
passed. Interface tests use a fixture; database tests use real temporary SQLite.
A complete installed Windows hardware/voice walkthrough has not been performed
for this build. The existing model has not been changed or re-benchmarked.

Update: back up the installed database, close Inventory Manager, and run the
2.0.7 installer against the same installation directory. Do not uninstall first.

Fixes a reproduced Windows startup failure caused by comparing schema CRLF/LF text literally. Only line endings are normalized; database contents and schema are unchanged. Regression tests verify LF/CRLF open, backup/restore, legacy migration and rejection of a genuinely removed trigger.
