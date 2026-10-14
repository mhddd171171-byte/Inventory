# Implementation and data flow

The existing Tauri 2, React/TypeScript and SQLite application was adapted to the confirmed single-user scope. Following the customer's additional search request, the frontend has seven pages: Dashboard, Products, Stock Entry, Stock Out, Search, Reports and AI Assistant. Backup/restore is a sidebar dialog. Authentication, roles, repair-order/customer/supplier/location/scanner UI and old write endpoints were removed.

## Inventory

`store.rs` implements parameterized approved actions with SQLite immediate transactions. Schema v2 has products, stock entries, stock sales, sale lines and FIFO allocations. Immutable batches retain purchase prices. Multi-line Stock Out validates all products and commits deductions, costs, revenue and profit together. Integer baisa and bounded arithmetic prevent floating-point rounding and overflow. Balances cannot become negative. Same-CR later transactions are allowed.

Opening an old database saves a complete pre-migration backup, preserves product identities/balances and business history, maps old nonnumeric SKUs to numeric Item Codes, and removes credentials from the active schema. Missing legacy prices/costs are nullable. Restore checks schema, integrity, foreign keys and FIFO consistency, then uses the online backup API with a recovery copy.

The Search page filters approved snapshots without adding database endpoints. Exact numeric CR and Item Code filters apply to the same sale line, with Arabic/English normalization for text. Date bounds are inclusive local calendar dates. Export refreshes the snapshot and uses the same validated filters. Monetary totals use integer baisa; unknown legacy amounts remain Unknown. PDF uses bundled fonts; multi-table Excel gives each table its own filtered sheet.

## Local assistant

Inventory and inference use separate backend locks. SQLite is read into a fresh snapshot, then its lock is released before the model runs. The local semantic planner receives the current message and bounded session context. Validated operations search products, inspect stock/history, summarize sales/profit/CR/value, or explain actual application workflows. Facts are formatted from approved database results. The LLM has no SQL/OS/write tools.

Search handles partial name/model/code, bilingual aliases, normalized Arabic/English digits, common abbreviations and conservative similar matches. Broad/approximate candidates require confirmation. Session memory holds selected products/groups, candidates, last searches, CRs, pending clarifications and recent turns; totals use all matches before display truncation. Restore clears all conversation contexts.

The model server is an owned hidden process on a random keyed loopback port. Its client disables proxies and redirects. Cached status allows the UI to update during startup/inference. A background supervisor retries crashes without blocking inventory; shutdown touches only owned children. Executables and models are resolved relative to Tauri resources, with persisted settings independent of the development checkout.

Validated social plans support greetings, thanks, farewell and listening/acknowledgment without clearing product context. A deterministic brief response is derived from the same approved data as the full answer. The frontend displays and speaks the brief response; the full text is available under More details. No second language-model rewrite can alter numerical facts.

## Voice

A packaged Python worker contains all dependencies. CPU-int8 multilingual Whisper transcribes in-memory audio after local VAD detects two seconds of silence. The 90-second safety limit reports an incomplete turn. English Kokoro-82M q8, Arabic Nabra-82M FP32 and CATT-EO int8 remain unchanged from 2.0.1. Shared locked eSpeak initialization and explicit UTF-8 protect bilingual synthesis.

Interruptible playback uses one 48 kHz full-duplex PortAudio stream with 10 ms blocks. The 24 kHz voice waveform is upsampled; a dedicated monitor processes 16 kHz microphone/reference frames through bundled WebRTC echo cancellation and sustained speech detection. An 800 ms clean pre-roll preserves initial words. On interruption the next output callback is silent, queued output is cleared, and the same microphone stream captures the full question. A progress event updates the UI while the backend is still listening. The final validated transcript becomes the next turn directly; it is not recorded a second time. Automatic input language can differ from the previous reply's language. Normal playback drains all speech and its tail. Audio remains in RAM. End/navigation/close stop only the owned worker. Models load on explicit Voice Mode activation; no network access is allowed.

## Platforms

Core Rust/SQLite/React code is shared. Runtime executable names and resource locations are platform-specific. Windows packages the CPU llama.cpp binaries, voice worker, model files and offline WebView2 installer. Intel macOS has native build/staging scripts and microphone entitlements; no macOS compatibility result is claimed until native/physical testing is available.

Version 2.0.3 makes existing buying/selling prices explicit. Shared incoming-batch tables and exports show exact BigInt batch totals, retaining Unknown costs. The model can map CRR/receipt IDs to CR lookup and product price questions to read-only movement history. CR replies include actual buying and selling totals; FIFO explanations use a validated help plan. No database schema or write operation was added.
