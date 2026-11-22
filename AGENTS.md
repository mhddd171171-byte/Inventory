# Project instructions

- Work only in this Inventory repository unless the user explicitly identifies another repository.
- The owner authorized a central online database on this feature branch: all installations have equal full access, without accounts, profiles or roles. Keep AI/voice local, preserve local verified backups, and never fall back to local inventory writes during disconnection. Archived Google Drive experiments remain excluded.
- Use descriptive `feature/`, `fix/`, `maintenance/`, `release/` or `archive/` branch names. Do not put assistant/tool branding in branch names; this is the owner's explicit preference.
- Preserve previous release tags, installers, business records and recovery backups. Do not rewrite history or delete historical releases as routine cleanup.
- The Windows and Intel Mac builds have native platform packaging, with shared inventory behavior and SQLite format. Use the appropriate release branch and verify relevant changes on each platform.
- Documentation-only maintenance must not trigger unnecessary multi-gigabyte installer rebuilds. Use a skip-CI commit for documentation/branch-organization changes when no runtime code changes.

## Dedicated Limited Access copy
The latest owner instruction authorizes this separate Windows-only staff edition. It has only Products and Search and no inventory write, restore, import, export or AI commands. Use the existing Turso database and token; do not modify its schema or any business rows. Only whitelisted display fields may be fetched, returned or cached. Seed the installer with a sanitized projection of the owner backup, never the full financial database or credentials. Keep the full-access worktree, installers, app identity and local data untouched.
