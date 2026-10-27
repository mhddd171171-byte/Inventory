# Repository organization

## Branches

- `macos-intel`: current default source and Intel Mac packaging. Shared inventory logic is kept aligned with the Windows release.
- `release/macos-2.0.11` and `release/windows-2.0.11`: current production release sources.
- `release/macos-2.0.10` and earlier Windows release branches: preserved historical versions; never merge blindly over current work.
- `archive/drive-backup-macos` and `archive/drive-backup-windows`: cancelled experimental online-backup work. These branches are not customer releases and must not be merged into production without an explicit request.

Use `feature/<topic>`, `fix/<topic>`, or `maintenance/<topic>` for new work. Use product/topic names rather than assistant or tool branding in branch names.

## Files

| Location | Contents |
| --- | --- |
| `src/` | React application screens, themes and reports |
| `src-tauri/src/` | Native application, SQLite ledger, backups and local assistant |
| `src-tauri/migrations/` | Validated database schemas and compatibility history |
| `src-tauri/resources/` | Bundled resource configuration and redistribution notices |
| `voice/` | Offline voice worker and pinned model configuration |
| `scripts/` | Native build, provisioning and verification tools |
| `tests/` | Frontend workflow fixtures and automated checks |
| `docs/` | Customer/developer guides, release notes and licenses |
| `.github/workflows/` | Native Intel Mac build and verification automation |

Runtime binaries/models are provisioned during the developer build and bundled into the installer. Never commit customer databases, personal backups, OAuth credentials or user tokens. Production uses local automatic backups only.

## Release history

Keep published tags, installer assets and their verification records. Documentation-only organization changes do not replace or rebuild an already verified installer. Use the release tag to identify the exact source of a published build.
