# Implementation and data flow

The existing Tauri 2, React/TypeScript and SQLite application was adapted to the confirmed single-user scope. The frontend has six pages: Dashboard, Products, Stock Entry, Stock Out, Reports and AI Assistant. Backup/restore is a sidebar dialog. Authentication, roles, repair-order/customer/supplier/location/scanner UI and old write endpoints were removed.

## Inventory

`store.rs` implements parameterized approved actions with SQLite immediate transactions. Schema v2 has products, stock entries, stock sales, sale lines and FIFO allocations. Immutable batches retain purchase prices. Multi-line Stock Out validates all products and commits deductions, costs, revenue and profit together. Integer baisa and bounded arithmetic prevent floating-point rounding and overflow. Balances cannot become negative. Same-CR later transactions are allowed.

Opening an old database saves a complete pre-migration backup, preserves product identities/balances and business history, maps old nonnumeric SKUs to numeric Item Codes, and removes credentials from the active schema. Missing legacy prices/costs are nullable. Restore checks schema, integrity, foreign keys and FIFO consistency, then uses the online backup API with a recovery copy.

## Local assistant

Inventory and inference use separate backend locks. SQLite is read into a fresh snapshot, then its lock is released before the model runs. The local semantic planner receives the current message and bounded session context. Validated operations search products, inspect stock/history, summarize sales/profit/CR/value, or explain actual application workflows. Facts are formatted from approved database results. The LLM has no SQL/OS/write tools.

Search handles partial name/model/code, bilingual aliases, normalized Arabic/English digits, common abbreviations and conservative similar matches. Broad/approximate candidates require confirmation. Session memory holds selected products/groups, candidates, last searches, CRs, pending clarifications and recent turns; totals use all matches before display truncation. Restore clears all conversation contexts.

The model server is an owned hidden process on a random keyed loopback port. Its client disables proxies and redirects. Cached status allows the UI to update during startup/inference. A background supervisor retries crashes without blocking inventory; shutdown touches only owned children. Executables and models are resolved relative to Tauri resources, with persisted settings independent of the development checkout.

## Voice

A packaged Python worker contains all dependencies. CPU-int8 multilingual Whisper transcribes in-memory microphone audio after local VAD detects an utterance end. English Piper and Arabic Nabra synthesize replies locally. Microphone capture is closed during transcription/playback to prevent hearing the generated reply. The frontend repeats listen → assistant → speak until End Conversation. Cancellation kills only its worker and unblocks pending IPC. Models initialize when Voice Mode starts and are released when it ends. The worker rejects network operations.

## Platforms

Core Rust/SQLite/React code is shared. Runtime executable names and resource locations are platform-specific. Windows packages the CPU llama.cpp binaries, voice worker, model files and offline WebView2 installer. Intel macOS has native build/staging scripts and microphone entitlements; no macOS compatibility result is claimed until native/physical testing is available.
