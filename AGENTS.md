# Project instructions

- Work only in this Inventory repository unless the user explicitly identifies another repository.
- Keep the application offline with local automatic backups. The archived online-backup experiments are not part of production.
- Use descriptive `feature/`, `fix/`, `maintenance/`, `release/` or `archive/` branch names. Do not put assistant/tool branding in branch names; this is the owner's explicit preference.
- Preserve previous release tags, installers, business records and recovery backups. Do not rewrite history or delete historical releases as routine cleanup.
- The Windows and Intel Mac builds have native platform packaging, with shared inventory behavior and SQLite format. Use the appropriate release branch and verify relevant changes on each platform.
- Documentation-only maintenance must not trigger unnecessary multi-gigabyte installer rebuilds. Use a skip-CI commit for documentation/branch-organization changes when no runtime code changes.
