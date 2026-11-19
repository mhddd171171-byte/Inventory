# Online inventory development status

19 November 2026 — work in progress on `feature/shared-online-inventory`.

Matching native 2.1.0 installers are built for owner validation; the Intel DMG is published as a prerelease. Existing 2.0.11 installers and release branches are unchanged. The owner-created Turso test endpoint is connected; no paid resource or automated customer-data migration has been created. The owner is now testing business records there: do not run destructive live fixtures or reset that database.

## Implemented development baseline

- Same full access for every installation; no user accounts or role hierarchy.
- Rust desktop gateway with HTTPS, native OS credential storage, persistent connection settings and background inventory refresh.
- No local writable inventory fallback. Local `.db` files are verified backup downloads.
- Separate authenticated Rust service reusing the existing validated SQLite business engine.
- Copy-on-write staging publishes business changes and durable operation receipts atomically through SQLite's backup transaction. Writes are serialized by one server owner, with an exclusive filesystem lock preventing accidental second ownership.
- Revision conflict detection, persistent retry identifiers, restore epoch invalidation and cross-device restore confirmation.
- Warranty action with original/new CRs, zero-price FIFO replacement, linked history, eligibility checks, adjusted lifetime original profit and separately dated financial costs.
- Warranty-aware history, search and PDF/Excel report models. Existing personal use, damage and damaged-return workflows retained.

## Provider change: Turso libSQL connection implemented and tested

The owner rejected Oracle because signup requests a payment method. Do not purchase or provision Oracle resources.

Turso's official pricing page checked 19 November 2026 advertises signup without a credit card, a Free plan with 5 GB storage, 500 million rows read/month and 10 million rows written/month. Pricing and allowances remain subject to the provider's terms. Do not enable paid overages or paid plans without explicit owner authorization.

The owner created `inventory-test` in Mumbai and uploaded the empty release schema. The application now connects directly to its libSQL SQL-over-HTTP endpoint through the native Rust backend. A separate paid server is not required for this adapter. The earlier persistent-server service remains a tested alternative; it is not the current Turso deployment path.

Live connectivity, schema checks, backup export, stock entry, sale, warranty, exact adjusted profit, retry replay, competing final-unit sales, cross-device refresh and restore were tested against the empty test database. Temporary business test records were removed and the verified empty baseline restored after every live scenario.

Turso's read-only `user_version` and `application_id` pragmas are not changed remotely. Compatibility is checked against the actual table/view/index/trigger definitions. Cloud metadata lives in `migration_info`; local portable backups retain the application's expected SQLite header/schema.

The native backend builds a validated private temporary snapshot from an atomic cloud read, runs the existing domain engine, then commits only changed rows in a cloud transaction guarded by revision and epoch. Original FIFO rowids are retained. Every write includes its durable idempotency receipt in that transaction. Conditional batches skip subsequent statements after any failure; commit is never sent after a failed write chunk. Cloud credentials are unavailable to the LLM and frontend status responses. All installations have equal full database access, as requested; the private read/write key must remain private.

Warranty is an action inside each product-line box, directly below Personal use/Damage. It opens with that line's selected item and quantity; a successfully submitted warranty consumes the draft line so it cannot accidentally be submitted as a paid sale.

Connection controls are in Settings only. The regular workspace retains clear connection-failure/stale-data notices. Import/restore uploads a validated .db backup and REPLACES the shared online records for all connected devices after explicit confirmation; it does not merge. The source file remains unchanged. A pre-restore recovery backup is saved, and the new epoch invalidates old pending requests.

For normal refresh, only the small cloud revision/epoch state is read when unchanged. After restart, a local backup may supply the complete snapshot only AFTER the cloud confirms the matching revision/epoch, and after its endpoint, checksum, schema and metadata are verified. Corrupt/mismatched caches trigger a fresh cloud read. Failed authentication/connectivity never grants access through a local writable fallback. Search/report/AI inputs remain complete. Automatic backups retain 64 recent versions and daily snapshots for 30 days; only owned UUID-named automatic files are rotated, never manual/recovery copies. Online mutations use Turso's clock normalized to Oman (+04:00); reporting month selection follows Asia/Muscat.

The user supplied a test token in chat. It was saved in Windows Credential Manager, not source/configuration/logs/installers. Rotate that token before live customer use, then enter its replacement in Settings. Do not copy the chat token into GitHub secrets or public files. The public test endpoint in opt-in tests is not a credential.

Official sources:

- https://turso.tech/pricing
- https://docs.turso.tech/libsql
- https://docs.turso.tech/cloud/limitations
- https://docs.turso.tech/sdk/ts/reference

## Validation and unfinished release gates

Verified locally: 79 desktop backend tests passed (2 ignored hardware/model cases), the existing server/domain/HTTP tests passed, 26 frontend tests and TypeScript/Vite production build passed. Three opt-in live Turso scenarios passed, plus an extended live cache/restart/corruption scenario. The actual Windows desktop build launched against Turso, displayed an empty dashboard and zero-price Warranty dialog, created a verified backup, and showed connection controls only inside Settings without revealing the key. Intel Mac checks are configured separately; no physical customer Mac testing is claimed.

Local test logs are ignored development artifacts (`server-test.log`, `native-test.log`, `frontend-test.log`, `frontend-build.log`). Backend tests cover warranty financial examples, replacement chains, portable backups, competing final-unit sales, lost-response replay and restore invalidation. A Windows integration test starts the real HTTP service and two actual desktop gateways, verifies OS-stored credentials and shared data, then checks disconnection behavior. It deletes only its own test credentials and terminates only its owned child process.

Release validation results and remaining limits:

1. Windows and Intel Mac platform checks passed on the performance source and corrected packaging source. The final Windows EXE installer built successfully; the native desktop was tested in an isolated profile against live Turso. This final installer has not been installed over the owner's existing application.
2. Intel Mac build 44436766429 passed strict mounted DMG/signature/resource checks, 17 exact model checks, voice-worker initialization, installed launch, automatic local AI readiness (18.52 seconds), clean exit/owned-helper cleanup and unchanged pre-existing local database bytes. The native Keychain roundtrip also passed. Live authenticated Turso operations on Mac, the physical customer Mac and microphone/speaker quality remain untested; no completely disconnected customer walkthrough is claimed.
3. Fresh actual customer backup and owner-triggered migration still require business-total reconciliation. No customer records have been uploaded by the connection tests. Do not restore over the owner's now-active cloud records merely to connect another installation.
4. Rotate the shared-in-chat token before customer rollout and configure installations through Settings.
5. Monitor provider quotas and local backup recency, and document provider-side recovery limits. The free provider's retention is separate from desktop backup retention.

Windows installer: `Inventory Manager_2.1.0_x64-setup.exe`. Intel installer: `Inventory Manager_2.1.0_x64.dmg`. Both use the same application logic and native OS executables. The existing release/default branches and stable installers remain unchanged. Mac build: https://github.com/mhddd171171-byte/Inventory/actions/runs/44436766429 . Published DMG parts must be assembled and hash-verified into one file before customer delivery.

The old stable local-only version remains usable separately. After an eventual online cutover, do not record new business transactions in both the old offline version and online version.

Latest review also preserves pre-correction/deletion/warranty recovery snapshots outside temporary staging and verifies original-CR warranty profit in both conversational answers and workspace search without attributing the paid sale revenue to the replacement CR. Windows and Intel Mac platform CI passed on commit 9ff54a8. A dedicated Intel installer workflow reuses hash-pinned, previously verified offline resources and checks the mounted package and installed app; packaging results must be checked before customer delivery.

## Save latency optimization

The cloud remains authoritative: a local-first success acknowledgement would allow competing computers to oversell stock. Saves now reuse the pooled HTTPS client, validate a checksum-verified cached snapshot against a small cloud revision/clock query, calculate the row delta locally, and publish it with an atomic revision/epoch guard and durable receipt. A normal warm save uses one metadata request and one guarded publish request. The UI and automatic backup are generated from the validated working state only after cloud confirmation, eliminating the redundant full download and immediate refresh request. Other-device changes invalidate the cache; complete search/report data is retained.

Read-only measurements against the actual Mumbai endpoint found roughly 0.8 seconds for a new connection and 0.2 seconds for a reused connection; these are metadata timings, not a promise of total save latency. Four isolated protocol tests cover a single atomic publish without readback, competing cached sales, rollback on a mid-batch failure and replay after a lost commit response. All passed. The revision-guard SQL was checked against the live server with EXPLAIN only, and the optimized Windows desktop loaded a current verified snapshot and backup without modifying records. No new write benchmark has been run on the owner's now-active database.

Pending writes retain their request identifier after an uncertain response; disconnected saves do not become an offline queue of authoritative stock changes. A failed local backup warns without falsely reporting that a confirmed cloud commit failed, and background refresh retries backup creation with a bounded interval.

Packaging verification caught Git line-ending normalization of two byte-pinned voice metadata files. Exact asset encodings are now preserved, and resource staging repairs any metadata mismatch from the hash-verified baseline. The corrected Intel build passed. Windows resource checks also verified all 16 voice manifest entries before repackaging. Models and voice behavior were not changed.

For direct Turso connections, application recovery snapshots are local files in the desktop backup folder. Provider-side retention is separate and is not an application-managed remote backup service; keep an additional copy on another disk.
