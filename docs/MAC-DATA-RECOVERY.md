# Mac 2.0.6 data protection and recovery

The customer has not yet supplied the Mac or its logs. The actual reason for the
missing application and apparently missing data is NOT confirmed. No customer
records have been recovered or inspected during this work.

Confirmed findings from source and prior package verification:
- The 2.0.4/2.0.5 app did not prevent use directly from a mounted DMG. That app
  location disappears when the image is ejected or after restarting the Mac.
- Previous backups required a manual action. There was no per-save backup.
- The live database is outside the app at
  ~/Library/Application Support/com.repaircenter.inventorymanager/data/inventory.db
- The app contains no self-uninstall or application-bundle deletion code.
- The existing releases are ad-hoc signed, not Developer ID signed/notarized.
  Gatekeeper verification warnings and an actual malware/removal warning are
  different events. Security cannot be named as the cause without the message.

The Mac 2.0.6 update:
- Refuses to open the database or start AI when a distributed .app is run outside
  /Applications or the user's ~/Applications folder. It shows installation steps.
  Cargo development binaries continue to work.
- Copies all committed product edits, Stock Entry and Stock Out changes to
  versioned standalone .db backups using the approved SQLite backup API.
- Uses ~/Inventory Manager Backups, independently of the app and its live-data
  folder. Keeps the latest 30 snapshots plus one per recent backup day (30 days
  with snapshots). Manual/pre-restore files are not rotated by this feature.
- Settings and Backup & restore show the version, paths, latest backup, warnings,
  Back up now, Open backup folder, and an optional additional USB/folder copy.
- A missing additional drive does not get silently recreated on the internal
  disk. The primary copy continues; a visible warning asks for reconnection.
- A committed business save remains successful if its backup fails. The UI
  explicitly warns that inventory was saved but a backup failed, avoiding an
  accidental retry and duplicate transaction.
- If the live DB is missing but backup history exists, preserves recovery state
  across restarts and requires a manual validated restore before new writes.
- If orphaned SQLite WAL/SHM recovery files exist without the main DB, stops
  startup rather than initializing over that evidence.
- Protects the existing DB immediately on upgrade; no schema or identifier change.
  Automatic .db backups remain compatible with Windows 2.0.4 manual Restore.
- Leaves the original Windows application and installer unchanged.

Backup copies on the same disk do not protect against loss/failure of that disk.
Use the optional second folder on another drive and/or normal Time Machine.
New backups protect future changes; they cannot recreate already lost records
that were never backed up. Reinstalling is not a data-recovery operation.

Security limitation:
Developer ID signing and Apple notarization require the owner's configured Apple
credentials. Without them, the installer remains ad-hoc signed and cannot be
claimed to eliminate every macOS security warning. Security checks are never
removed or disabled by this update.

Apple references:
https://support.apple.com/guide/mac-help/install-and-uninstall-other-apps-mh35835/mac
https://support.apple.com/en-us/102445
https://support.apple.com/guide/security/protecting-against-malware-sec469d47bd8/web
