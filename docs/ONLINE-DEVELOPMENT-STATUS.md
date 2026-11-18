# Online inventory development status

19 November 2026 — work in progress on `feature/shared-online-inventory`.

This is not a customer release. Existing 2.0.11 installers, release branches and customer databases are unchanged. No online account, paid resource, production endpoint or customer-data migration has been created.

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

Connection controls are in Settings only. The regular workspace retains clear connection-failure/stale-data notices. Import/restore uploads a validated .db backup and REPLACES the shared online records for all connected devices after explicit confirmation; it does not merge. The source file remains unchanged. A pre-restore recovery backup is saved, and the new epoch invalidates old pending requests.

For normal refresh, only the small cloud revision/epoch state is read when unchanged. After restart, a local backup may supply the complete snapshot only AFTER the cloud confirms the matching revision/epoch, and after its endpoint, checksum, schema and metadata are verified. Corrupt/mismatched caches trigger a fresh cloud read. Failed authentication/connectivity never grants access through a local writable fallback. Search/report/AI inputs remain complete. Automatic backups retain 64 recent versions and daily snapshots for 30 days; only owned UUID-named automatic files are rotated, never manual/recovery copies. Online mutations use Turso's clock normalized to Oman (+04:00); reporting month selection follows Asia/Muscat.

The user supplied a test token in chat. It was saved in Windows Credential Manager, not source/configuration/logs/installers. Rotate that token before live customer use, then enter its replacement in Settings. Do not copy the chat token into GitHub secrets or public files. The public test endpoint in opt-in tests is not a credential.

Official sources:

- https://turso.tech/pricing
- https://docs.turso.tech/libsql
- https://docs.turso.tech/cloud/limitations
- https://docs.turso.tech/sdk/ts/reference

## Validation and unfinished release gates

Verified locally: 78 desktop backend tests passed (2 ignored hardware/model cases), the existing server/domain/HTTP tests passed, 25 frontend tests and TypeScript/Vite production build passed. Three opt-in live Turso scenarios passed, plus an extended live cache/restart/corruption scenario. The actual Windows desktop build launched against Turso, displayed an empty dashboard and zero-price Warranty dialog, created a verified backup, and showed connection controls only inside Settings without revealing the key. Intel Mac checks are configured separately; no physical customer Mac testing is claimed.

Local test logs are ignored development artifacts (`server-test.log`, `native-test.log`, `frontend-test.log`, `frontend-build.log`). Backend tests cover warranty financial examples, replacement chains, portable backups, competing final-unit sales, lost-response replay and restore invalidation. A Windows integration test starts the real HTTP service and two actual desktop gateways, verifies OS-stored credentials and shared data, then checks disconnection behavior. It deletes only its own test credentials and terminates only its owned child process.

Still required before customer delivery:

1. Native Intel Mac build and integration checks, including Keychain behavior; CI platform checks are configured without cloud credentials.
2. Final installed update tests on both OSes and production packaging with the unchanged offline AI/voice resources.
3. Fresh actual customer backup and owner-triggered migration with business-total reconciliation. No customer records have been uploaded by the connection tests.
4. Rotate the shared-in-chat token before customer rollout and configure installations through Settings.
5. Monitor provider quotas and local backup recency, and document provider-side recovery limits. The free provider's retention is separate from desktop backup retention.

This remains unreleased development version 2.1.0. No final Windows/Mac installer is claimed yet. The existing release/default branches and installers remain unchanged.

The old stable local-only version remains usable separately. After an eventual online cutover, do not record new business transactions in both the old offline version and online version.
