# UI workflow regression checks

`ui-flow.mjs` starts a dedicated Vite server on `127.0.0.1:1433`, opens headless Microsoft Edge, and interacts with the actual React application. The fixture installs Tauri's official `mockIPC` only before importing the application from `ui-fixture.ts`. Neither fixture is imported by `src/main.tsx` or included in the production build. The mock does not verify Rust/database behavior; native backend tests and installer smoke testing cover that separately.

The tests do not add seed records to the user's database, show a browser window, or install runtime components. Screenshots, test results, and real generated PDFs are written to ignored `tmp/ui/` for review.

The test Vite configuration keeps an independent dependency cache and prebundles the dynamically imported PDF libraries. This prevents development-server dependency optimization from reloading the fixture halfway through a report check.

Install Playwright as a development dependency in a development environment, or point `PLAYWRIGHT_MODULE` to an already installed Playwright package. Example in PowerShell:

```powershell
$env:PLAYWRIGHT_MODULE='C:/path/to/node_modules/playwright'
node tests/ui-flow.mjs
```

The default browser is installed Microsoft Edge. Set `UI_BROWSER_CHANNEL=chrome` to use installed Chrome, or change the launch configuration to use locally installed Playwright Chromium. `UI_TEST_PORT` overrides the server port.

Coverage:

- Direct single-user dashboard, with no login or account routes.
- Numeric Item Code validation, Arabic product names/models, exact baisa amounts and manual product creation.
- Existing-code automatic product fill, independent incoming FIFO batches and available quantity.
- Mandatory numeric CR and selling-price validation, multi-product atomic Stock Out, repeated CR, cost/revenue/profit and stock alerts.
- Invalid input preserves form state and saves nothing.
- Partial name/model/Arabic searches and product transaction details.
- Three report types exported to both genuine PDF and Excel byte payloads.
- Separate Search navigation, exact CR plus Item Code, name/model/date filters and matching PDF/Excel exports.
- Short friendly chat, an interrupted reply followed by its captured transcript, and spoken farewell ending Voice Mode.
- Conversation continuity across navigation and a two-turn Voice Mode control loop, including End during listening.
- Local backup dialog and responsive 960px layout.
- No browser errors or outbound network requests.

The IPC fixture verifies the actual React interface and its control flow. It does not substitute for Rust SQLite tests, actual local model evaluations, packaged speech checks, or installed-EXE testing. Those results are recorded separately in `docs/VALIDATION.md`.
