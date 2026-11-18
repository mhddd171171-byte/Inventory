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

## Provider change: Oracle rejected; Turso under evaluation

The owner rejected Oracle because signup requests a payment method. Do not purchase or provision Oracle resources.

Turso's official pricing page checked 19 November 2026 advertises signup without a credit card, a Free plan with 5 GB storage, 500 million rows read/month and 10 million rows written/month. Pricing and allowances remain subject to the provider's terms. Do not enable paid overages or paid plans without explicit owner authorization.

Turso Cloud offers both libSQL and the newer Turso engine. libSQL is the preferred compatibility candidate for this existing SQLite application. The current development service uses local persistent server storage; it is NOT yet a Turso driver. Do not deploy it on ephemeral free hosting and assume that data is persistent.

Important verified differences in Turso Cloud: `PRAGMA user_version` and `application_id` are read-only; `busy_timeout` and `journal_mode` are not managed as in local SQLite. The existing schema initializer, strict backup/schema validator, connection driver, transaction coordination and portable `.db` backup generation must be adapted and tested against the selected Turso engine. Merely replacing a URL will not work. Do not upload the customer's database before a compatibility/migration test succeeds.

The database service and the approved business-operation layer are distinct. Selecting Turso does not by itself host the existing Rust HTTP service. The final deployment needs either a Turso-compatible business layer on suitable hosting or a thoroughly validated native connection adapter. Keep database owner credentials out of generic/public installers and out of AI tools.

Official sources:

- https://turso.tech/pricing
- https://docs.turso.tech/libsql
- https://docs.turso.tech/cloud/limitations
- https://docs.turso.tech/sdk/ts/reference

## Validation and unfinished release gates

Verified locally: 77 desktop backend tests passed (2 ignored hardware/model cases), 46 server/domain/HTTP integration tests passed, 25 frontend tests passed, and the TypeScript/Vite production build passed. Intel Mac and live Turso testing have not been performed.

Local test logs are ignored development artifacts (`server-test.log`, `native-test.log`, `frontend-test.log`, `frontend-build.log`). Backend tests cover warranty financial examples, replacement chains, portable backups, competing final-unit sales, lost-response replay and restore invalidation. A Windows integration test starts the real HTTP service and two actual desktop gateways, verifies OS-stored credentials and shared data, then checks disconnection behavior. It deletes only its own test credentials and terminates only its owned child process.

Still required before customer delivery:

1. Confirm Turso account and engine availability; verify compatibility on a separate empty/test database.
2. Implement and test the Turso adapter and its hosting model, including atomic concurrent writes and idempotency on the authoritative remote database.
3. Complete and test backup retention, monitoring and independent server/cloud recovery for the selected deployment.
4. Native Intel Mac build and integration tests, including Keychain behavior.
5. UI acceptance tests for connection configuration, stale data and warranty; native installed tests on both OSes.
6. Fresh actual customer backup, validated migration and business-total reconciliation.
7. Matching production installers provisioned for the selected online service. The source version 2.1.0 identifies this development work; no 2.1.0 production release is claimed.

The old stable local-only version remains usable separately. After an eventual online cutover, do not record new business transactions in both the old offline version and online version.
