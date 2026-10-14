# Windows offline installer

The supported Windows build is `scripts/build-windows.ps1` (or `npm run desktop:build`). It compiles the existing Tauri app with `--no-bundle`, then uses Inno Setup 6.5.4 and `windows.iss` to produce one EXE containing AI, speech, licenses and Microsoft's offline WebView2 installer. Zip compression keeps build time and installer memory low. Customers need no build tools or Python.

The earlier NSIS template remains a documented build comparison: Tauri CLI 2.12.2's MIT/Apache-2.0 template with solid compression disabled. NSIS cannot contain the selected 4B model plus speech resources in one executable because of its size limit; do not use it for this release.

Inno Setup's reviewed compiler license is bundled under `resources/licenses/installer/`. The installer uses per-user application paths and never installs, deletes or overwrites the separate SQLite data directory. Legacy uninstall registration is removed only after verifying that it belongs to the same installation directory.

Source: https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v2.12.2/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi

NSIS issue: https://sourceforge.net/p/nsis/bugs/1284/
