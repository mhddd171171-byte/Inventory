# Inventory Manager
### Al Mohtarif Repair Center

Offline inventory management for **Windows** and **Intel Mac**. Products, FIFO purchase batches, customer CR sales, damaged returns, PDF/Excel reports, local AI and bilingual voice run on the customer's computer.

**Current verified release: 2.0.11.** Local automatic backups only; no Google Drive integration, cloud AI service or paid AI API is required.

## Installers

- **Windows:** `Inventory Manager_2.0.11_x64-setup.exe`
- **Intel Mac, macOS 13+:** `Inventory Manager_2.0.11_x64.dmg`

Customers receive one installer for their operating system. No programming IDE, Terminal, Python installation, Ollama or startup model downloads are required.

[Verified Intel Mac release and checks](https://github.com/mhddd171171-byte/Inventory/releases/tag/macos-v2.0.11-build40596634606-1) · [Release notes](docs/RELEASE-2.0.11.md) · [Installation guide](docs/INSTALLATION.md)

GitHub's large-file limit means the Mac release stores the DMG as numbered parts with `INSTALLER-PARTS.json`. Those parts are for the distributor to assemble and verify; give the customer the resulting **single DMG**. The ready Windows installer is distributed separately.

Before updating, back up the database and quit the app. Install over the existing Windows installation, or replace the Mac app in Applications. Updates preserve the database and settings. New-format backups require 2.0.11 on both computers; older backups can be restored and upgraded. Restore replaces records rather than merging.

## What's in 2.0.11

- **Personal use** and **Damage** below Quantity on each Stock Out product line. Non-sale lines have no selling price; their FIFO buying costs are reported separately.
- **Damaged customer returns** inside history → Edit: refund, replacement or pending resolution, keeping the original CR and sale.
- Readable dark surfaces across tables, stock badges, forms, reports and edit windows.
- Existing item search, password-authorized edits/deletion, offline AI/voice and local automatic backups remain available.

## Source branches

| Branch | Purpose |
| --- | --- |
| [`macos-intel`](https://github.com/mhddd171171-byte/Inventory/tree/macos-intel) | Default branch: current shared application and Intel Mac packaging |
| [`release/macos-2.0.11`](https://github.com/mhddd171171-byte/Inventory/tree/release/macos-2.0.11) | Current verified Intel Mac release source |
| [`release/windows-2.0.11`](https://github.com/mhddd171171-byte/Inventory/tree/release/windows-2.0.11) | Current verified Windows release source and native packaging |
| `release/*-2.0.10` and earlier | Preserved previous versions |
| `archive/drive-backup-*` | Cancelled online-backup experiments; excluded from production |

[Repository layout and branch policy](docs/REPOSITORY-STRUCTURE.md)

## Developer setup

Use the Windows release branch to build Windows and the default branch to build Intel Mac. Native executables and resource staging differ by platform; shared business logic and database format are the same.

Windows prerequisites: Node.js 22+, Rust/MSVC, Visual Studio C++ Build Tools, Windows SDK and Inno Setup 6.5.4. See [Intel Mac build preparation](docs/MACOS-PREPARATION.md) for the native Mac workflow. Build-time provisioning may need internet; installed applications do not download their AI components.

```powershell
npm ci
npm test
npm run build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/desktop.ps1 test
npm run desktop:build
```

[User guide](docs/USER-GUIDE.md) · [AI and model licenses](docs/AI-LICENSES.md) · [Dependency notices](docs/DEPENDENCIES.md) · [Validation and limitations](docs/VALIDATION.md)

The assistant uses bounded read-only backend tools. It cannot execute arbitrary SQL or modify inventory. Microphone audio stays local. Customer databases, credentials and large runtime/model binaries are excluded from source control.

Native Intel Mac launch, database preservation, backup, local AI and synthesis checks passed on the build runner. The customer's physical Mac, microphone/speaker quality and a fully disconnected customer installation walkthrough remain untested. The Mac bundle is ad-hoc signed, not Apple-notarized.
