"""Test exact production backup/SQLite modules without linking the whole desktop UI."""
import json,os,shutil,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1]
probe=root/'.build/drive-native-check'
probe.mkdir(parents=True,exist_ok=True)
(probe/'src').mkdir(exist_ok=True)
(probe/'Cargo.toml').write_text('''[package]
name = "inventory-drive-native-check"
version = "0.0.0"
edition = "2021"
build = "build.rs"
[dependencies]
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
rusqlite = { version = "0.32", features = ["bundled", "backup"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
sha2 = "0.10.9"
base64 = "0.22.1"
[target.'cfg(windows)'.dependencies]
keyring = { version = "=3.6.3", features = ["windows-native"] }
[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "=3.6.3", features = ["apple-native"] }
[dev-dependencies]
tempfile = "3"
''',encoding='utf-8')
modules=['store','data_safety','cloud_backup']
for name in modules:
    original=root/'src-tauri/src'/f'{name}.rs'
    shutil.copy2(original,probe/'src'/original.name)
    children=root/'src-tauri/src'/name
    if children.is_dir():shutil.copytree(children,probe/'src'/name,dirs_exist_ok=True)
shutil.copytree(root/'src-tauri/migrations',probe/'migrations',dirs_exist_ok=True)
source='\n'.join(f'mod {name};' for name in modules)
(probe/'src/lib.rs').write_text(source,encoding='utf-8')
(probe/'build.rs').write_text('fn main(){std::fs::write(std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("google-oauth-client.json"),r#"{"client_id":"","client_secret":""}"#).unwrap();}',encoding='utf-8')
if not (probe/'Cargo.lock').exists():shutil.copy2(root/'src-tauri/Cargo.lock',probe/'Cargo.lock')
args=['cargo','test','--manifest-path',str(probe/'Cargo.toml')]
# Compilation finishes before Keychain is unlocked for the short credential tests.
subprocess.run(args+['--no-run'],check=True)
if os.environ.get('RUNNER_OS')=='macOS':
    import secrets
    keychain=Path(os.environ['RUNNER_TEMP'])/'drive-component-tests.keychain-db'
    password=secrets.token_hex(24)
    print('::add-mask::'+password,flush=True)
    for command in [['create-keychain','-p',password,str(keychain)],['set-keychain-settings','-lut','21600',str(keychain)],['unlock-keychain','-p',password,str(keychain)],['default-keychain','-s',str(keychain)],['list-keychains','-d','user','-s',str(keychain),'login.keychain-db']]:
        subprocess.run(['security',*command],check=True)
result=subprocess.run(args+['--','--nocapture'],timeout=180)
raise SystemExit(result.returncode)
