# Windows platform runtime exceptions

Reviewed for the 2.0.0 Windows package on **13 October 2026**.

The request specifies Windows and Tauri while asking for open-source components. The application source, React/Tauri/Rust application libraries, SQLite, selected Qwen weights, llama.cpp, and PDF/font components have the licenses recorded in [DEPENDENCIES.md](DEPENDENCIES.md), [AI-LICENSES.md](AI-LICENSES.md), and [REPORTS.md](REPORTS.md). The complete packaged Windows distribution also depends on the following proprietary Microsoft platform components, so it must not be described as entirely free/open-source software.

| Component | Use in this package | Licensing and deployment |
| --- | --- | --- |
| Windows 10/11 | Target operating system | Supplied/licensed separately by the computer owner; not included in this application installer. |
| Microsoft Edge WebView2 Runtime | Tauri's embedded desktop renderer | The full offline installer is embedded in the NSIS package. Microsoft's runtime terms apply. This is a desktop rendering dependency, not a requirement to open an Edge browser tab. See [Microsoft's offline deployment documentation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution#offline-deployment). |
| Microsoft Visual C++ Runtime | Required by the bundled Windows llama.cpp binaries | App-local x64 `msvcp140.dll`, `vcruntime140.dll`, and `vcruntime140_1.dll`, version `14.50.35719.0`, are copied unmodified beside the inference executable. Microsoft's runtime and redistribution terms apply. |

The selected C++ DLLs have valid Microsoft signatures; their per-file hashes are recorded in `src-tauri/resources/ai/SHA256SUMS.txt`. The license text is included as `src-tauri/resources/ai/LICENSE-Microsoft-Visual-C-Runtime.txt`. The matching locally available Microsoft redistributable package was used to identify the installed runtime version; ordinary users do not need a separate C++ runtime download for these app-local inference dependencies.

Microsoft's [Visual Studio redistribution list](https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution#visual-c-runtime-files) permits redistribution of the listed unmodified runtime files by licensed Visual Studio users subject to their applicable Visual Studio terms. Distribution rights must be established under those terms. The [Visual C++ runtime end-user license](https://visualstudio.microsoft.com/license-terms/vs2022-cruntime/) is not itself a grant of redistribution permission. Preserve the Microsoft notices and do not apply the project's MIT license to those binaries.

These components do not create a hosted inventory service or an application subscription. The inventory database and inference requests remain local. WebView2's separately managed Microsoft runtime can receive operating-system/runtime updates when the computer is online; the inventory app does not download its model, speech weights, or application dependencies during ordinary operation.
