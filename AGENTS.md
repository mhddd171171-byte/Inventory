# Project instructions

- Work only in this Inventory repository unless the user explicitly identifies another repository.
- The owner authorized a central online database on this feature branch: all installations have equal full access, without accounts, profiles or roles. Keep AI/voice local, preserve local verified backups, and never fall back to local inventory writes during disconnection. Archived Google Drive experiments remain excluded.
- Use descriptive `feature/`, `fix/`, `maintenance/`, `release/` or `archive/` branch names. Do not put assistant/tool branding in branch names; this is the owner's explicit preference.
- Preserve previous release tags, installers, business records and recovery backups. Do not rewrite history or delete historical releases as routine cleanup.
- The Windows and Intel Mac builds have native platform packaging, with shared inventory behavior and SQLite format. Use the appropriate release branch and verify relevant changes on each platform.
- Documentation-only maintenance must not trigger unnecessary multi-gigabyte installer rebuilds. Use a skip-CI commit for documentation/branch-organization changes when no runtime code changes.
