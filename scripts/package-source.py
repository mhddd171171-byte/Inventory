"""Package application source and redistribution notices without customer data."""
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "deliverables/InventoryManager-2.0.3-source.zip"
roots = ["src", "voice", "scripts", "docs", "tests", "src-tauri/src",
         "src-tauri/migrations", "src-tauri/capabilities", "src-tauri/icons",
         "src-tauri/installer", "src-tauri/resources/licenses"]
files = set()
for folder in roots:
    for p in (ROOT / folder).rglob("*"):
        if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc":
            files.add(p)
for name in [".gitignore", "README.md", "index.html", "package.json", "package-lock.json",
             "tsconfig.json", "vite.config.ts", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock",
             "src-tauri/build.rs", "src-tauri/tauri.conf.json", "src-tauri/tauri.macos.conf.json",
             "src-tauri/Info.plist", "src-tauri/Entitlements.plist"]:
    p = ROOT / name
    if p.is_file():
        files.add(p)
for folder in ["src-tauri/resources/ai", "src-tauri/resources/voice"]:
    for p in (ROOT / folder).rglob("*"):
        if p.is_file() and "runtime" not in p.parts and p.suffix in {".json", ".txt", ".md"}:
            files.add(p)
OUTPUT.parent.mkdir(exist_ok=True)
with zipfile.ZipFile(OUTPUT, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=1, strict_timestamps=False) as archive:
    for p in sorted(files):
        archive.write(p, p.relative_to(ROOT).as_posix())
    archive.writestr("SOURCE-PACKAGE.md", """# Inventory Manager 2.0.3 source

This archive contains the application, schemas, tests, build scripts, model provenance,
and dependency licenses/corresponding source. Customer databases, build caches and
development credentials are excluded. Native binaries and model weights are supplied
in the accompanying offline Windows installer rather than duplicated in this archive.

For an offline Windows rebuild, extract this archive, copy the installed application's
resources directory into src-tauri/resources, and follow README.md. The build machine
needs the documented Node/Rust/C++/Inno Setup toolchains; customer computers do not.
The original workspace also retains the complete staged runtime resources.

See docs/VALIDATION.md and the delivered VALIDATION.md for testing and remaining
physical Mac, clean offline Windows and human voice acceptance requirements.
""")
with zipfile.ZipFile(OUTPUT, "r") as archive:
    if archive.testzip() is not None:
        raise RuntimeError("Source archive validation failed")
print(f"{OUTPUT.name}: {len(files)} source/notice files, {OUTPUT.stat().st_size} bytes")
