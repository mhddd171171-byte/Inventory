# Inventory Manager 2.0.11 — Windows and Intel Mac

Built from the approved offline 2.0.10 release. No Google Drive or online-backup integration is included. Existing offline AI, voice, inventory search, Delete inside Edit, and local automatic backups remain.

## Stock Out

Each product line has **Personal use** and **Damage** checkboxes below Quantity. Only one reason can be selected on a line. Its selling price is cleared and disabled. With no normal sale lines, the CR is cleared and disabled too. A mixed transaction still requires a CR for its normal sale lines; personal/damage lines have no customer CR. Non-sale quantities reduce available stock using FIFO, and their buying costs are shown separately from sales in monthly exports.

## Damaged customer returns

Open **Stock Out → Stock Out history → Edit → Record damaged return**. Enter the returned quantity and fault, choose Refund, Replacement, or Pending, and authorize with password **1234**.

- Refund records the amount actually returned, no more than the original price of the returned units.
- Replacement searches by name/model/code and deducts the replacement quantity once. It stays on the original CR without a second sale charge. An additional payment or partial refund can be entered explicitly.
- Pending keeps the return open. Reopen Edit and choose **Resolve pending return** later.
- Defective returned units never re-enter sellable stock. The original sale remains. The same units cannot be returned twice. Return-linked records are protected from ordinary edits/deletion; a recovery copy is made before settlement.
- Refunds and additional payments affect reports on the settlement date. Replacements add FIFO buying cost without increasing the number of paid units sold. Personal use and damage costs are separate from sales gross profit.

## Dark theme

Dark table headers, stock badges, forms, history, reports, chat surfaces and edit/return windows use readable dark surfaces throughout the workspace.

## Install / update

Back up the database and quit Inventory Manager before updating. Windows: run the new setup EXE into the existing installation folder. Intel Mac (macOS 13+): open the DMG, drag Inventory Manager into Applications and choose Replace. Launch it from Applications and eject the DMG. Do not uninstall first or delete the application data directory.

The program upgrades the database to schema v3 with a recovery backup named `before-stock-dispositions-….db` in its data folder. Existing v1/v2 backups can be restored and upgraded without modifying the selected backup file. New v3 backups require this release on both computers; do not restore them into 2.0.10. Restoring replaces records rather than merging. Updates preserve data and settings and do not synchronize computers.

Native Mac signing remains ad hoc unless a Developer ID is provided. This does not certify Apple notarization. Physical customer hardware, audio quality, and a customer installation walkthrough need separate confirmation.
