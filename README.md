# Inventory Manager — Al Mohtarif Repair Center

The existing Tauri 2 / React / TypeScript / Rust / SQLite desktop application now uses the confirmed single-user workflow. It opens directly to the dashboard, with numeric Item Codes, FIFO stock batches, CR-linked multi-product Stock Out, exact integer baisa accounting, PDF/Excel reports, an offline conversational assistant, and continuous bilingual Voice Mode.

No login, cloud service, subscription, customer account, repair-order management, or scanner is required. The inventory interface remains usable while the bundled AI starts in the background.

## Install and use

Use the Windows EXE installer in `deliverables/`. The package includes the local AI runtime, selected model, packaged voice worker and speech models, and the offline WebView2 installer. See [installation](docs/INSTALLATION.md), [user guide](docs/USER-GUIDE.md), [validation and limitations](docs/VALIDATION.md), and [Intel macOS preparation](docs/MACOS-PREPARATION.md).

## Build from source

Windows build prerequisites: Node.js 22+, Rust stable/MSVC, Visual Studio C++ Build Tools, Windows SDK, and Inno Setup 6.5.4. Initial developer provisioning can require internet; the installed application never downloads models during startup.

```powershell
npm ci
npm run desktop:dev
npm run desktop:build
npm test
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/desktop.ps1 test
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/desktop.ps1 test-ai
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/desktop.ps1 test-voice
```

Retain supplied binary resources when building offline. `scripts/build-voice.ps1` packages the voice worker with its own Python runtime. Users do not install Python. Model provenance and redistribution notices are in [AI licenses](docs/AI-LICENSES.md); library notices are in [dependencies](docs/DEPENDENCIES.md) and `src-tauri/resources/licenses/`.

Version 2.0.3 clarifies buying/selling prices and batch totals, includes buying/selling amounts in CR voice replies and explains FIFO. It retains the Search workspace and speech interruption from 2.0.2, plus the improved bilingual voices from 2.0.1. [Search and conversation](docs/SEARCH-AND-CONVERSATION.md) and [voice evaluation](docs/VOICE-IMPROVEMENTS.md) describe those changes. `test-voice` audibly plays test speech; set `INVENTORY_LONG_VOICE_TEST=1` for long playback. Python regressions use `.build/voice-env/Scripts/python.exe -m unittest discover -s voice -p 'test_*.py'`. `scripts/provision-voice.py` validates pinned build-time downloads; the installed app never calls it.

Persistent data: `%LOCALAPPDATA%\com.repaircenter.inventorymanager\data\inventory.db`. Schema v2 is `src-tauri/migrations/002_single_user.sql`. Safe migration preserves existing product balances and business history, saves a pre-migration backup, and marks unavailable legacy costs or selling prices as unknown. Old credentials are removed from the active database. Backup/restore uses SQLite online backup and validates integrity, schema, relationships, and balances.

The model interprets a bounded read-only plan. Rust resolves references and searches current SQLite results, then formats facts. The LLM has no SQL, filesystem, shell, inventory-writing, or internet tools. Voice runs locally; during interruptible playback, echo cancellation separates the user's speech from the assistant's output. Microphone audio remains in memory.

Windows WebView2 and Microsoft runtime DLLs are proprietary platform dependencies documented in [platform runtimes](docs/PLATFORM-RUNTIMES.md). Intel macOS source/build preparation is included; final Mac compatibility and latency require testing on suitable macOS and the customer's physical Intel i5 MacBook.
