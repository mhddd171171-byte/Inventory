"""Collect exact locked Rust dependency notices for redistribution (run after cargo metadata)."""
import json
import shutil
from pathlib import Path

root = Path(__file__).resolve().parent.parent
metadata = json.loads((root / '.build/cargo-metadata.json').read_text(encoding='utf-8-sig'))
output = root / 'src-tauri/resources/licenses'
output.mkdir(parents=True, exist_ok=True)
lines = ['# Rust dependency notices', '', 'Exact Cargo.lock resolution. Unmodified upstream components. All selected license expressions permit commercial redistribution when their terms are preserved.', '', 'MPL-2.0 components have their complete published source included in source/ alongside these notices. Application source does not modify those components.', '']
missing = []
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package.get('source') is None:
        continue
    name, version = package['name'], package['version']
    base = Path(package['manifest_path']).parent
    license_files = [p for p in base.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT', 'UNLICENSE'))]
    if package.get('license_file'):
        p = base / package['license_file']
        if p.is_file() and p not in license_files:
            license_files.append(p)
    target = output / 'rust' / f'{name}-{version}'
    target.mkdir(parents=True, exist_ok=True)
    for file in license_files:
        shutil.copyfile(file, target / file.name)
    if 'MPL-2.0' in (package.get('license') or ''):
        shutil.copytree(base, output / 'source' / f'{name}-{version}', dirs_exist_ok=True)
    lines += [f'## {name} {version}', '', f"License: {package['license']}", f"Source: https://crates.io/api/v1/crates/{name}/{version}/download", f"Upstream: {package.get('repository') or package.get('homepage') or ''}", '']
    if not license_files:
        missing.append(f'{name} {version}')
        # The published Cargo metadata remains an auditable upstream attribution.
        shutil.copyfile(base / 'Cargo.toml', target / 'Cargo.toml')
    for file in license_files:
        lines += [f'### {file.name}', '', file.read_text(encoding='utf-8', errors='replace'), '']
(output / 'RUST-NOTICES.md').write_text('\n'.join(lines), encoding='utf-8')
print(f'Collected {len(metadata["packages"])} package records. Packages without a standalone notice: {missing}')
