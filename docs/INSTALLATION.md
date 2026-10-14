# Windows installation and offline operation

Use the 2.0.3 Windows x64 EXE installer in `deliverables/`. Close Inventory Manager before upgrading. Keep a database backup before moving to another computer. Installation files and persistent inventory data use separate directories, so an application upgrade does not replace the database.

1. Run the installer and select the installation location.
2. Launch Inventory Manager. The dashboard opens directly; there are no credentials.
3. The local model starts automatically. The indicator changes from **Starting AI...** to **AI Ready**. Inventory forms are available while it loads.
4. Open AI Assistant for text chat, or choose **Start Conversation** for Voice Mode. Speech models initialize automatically on activation. Choose **End Conversation** to release microphone, playback, and voice resources.

All inference and speech resources are bundled. No model download, API key, external server, Python installation, or repeated configuration is needed. The installer embeds Microsoft's offline WebView2 installer for the desktop renderer. An existing suitable WebView2 installation can be reused.

## Requirements

Windows 10/11 x64, an Intel/AMD processor supported by the bundled CPU runtime, at least 5 GB free disk space for installation plus space for backups, and enough free RAM for the OS plus inference. The selected 4B model used about 4.8 GB resident memory during the Windows evaluation; Voice Mode adds speech models while active. 16 GB system RAM is recommended for comfortable combined use, but lower-memory hardware must be benchmarked before deployment. The customer's Intel i5 Mac requirements remain to be measured; development-machine latency is not a promise for older hardware.

For voice, connect a working microphone and speakers. If Windows microphone privacy denies access, enable microphone access for desktop apps in Windows settings yourself. Text chat and inventory remain available without audio hardware. No Windows security or privacy setting is changed by the application.

## Data and backups

The database is `%LOCALAPPDATA%\com.repaircenter.inventorymanager\data\inventory.db`. Choose **Backup & restore** in the sidebar to create a local `.db` backup. Restore validates the selected backup and first creates a recoverable copy of the current database. A restore replaces the active inventory and clears assistant context.

Upgrading a v1 database makes a full backup before migration. Existing numeric references are retained; old nonnumeric SKUs receive numeric Item Codes with the mapping preserved. Business history is archived without login secrets. Historical missing costs/prices display **Unknown**, so the app does not fabricate old profits.

AI settings are persisted locally with installation-relative resources. A previous development path is discarded in favor of bundled files. A technical `performance_profile` of `low_memory` reduces context to 2048 tokens; it is not a customer setup requirement. The default profile uses 4096 tokens and at most four inference threads.

If the owned AI server crashes, the app attempts bounded automatic recovery. Closing the app shuts down only its own model and voice processes. No process-name-wide termination is used.

## Build and deployment

See the root README for developer commands. `npm run desktop:build` compiles Tauri and then the single-file Inno Setup offline installer; the full model/voice package exceeds NSIS's executable limit. Runtime binaries, models and the cached offline WebView2 installer must be staged before building. Provisioning occurs only during authorized development. The Windows installer is unsigned unless a deployment signing certificate is separately supplied. Preserve bundled licenses and source notices when redistributing it.

Intel macOS preparation and pending physical testing are documented in [MACOS-PREPARATION.md](MACOS-PREPARATION.md). No final Mac installation is claimed from the Windows build.
