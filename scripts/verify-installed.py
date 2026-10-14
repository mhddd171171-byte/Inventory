"""Read-only Windows release/resource/database verification for developers."""
import argparse
import hashlib
import json
import os
import sqlite3
from pathlib import Path

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--baseline',required=True)
parser.add_argument('--version',default=json.loads((root/'package.json').read_text(encoding='utf-8'))['version'])
args=parser.parse_args()
installed=Path(os.environ['LOCALAPPDATA'])/'Programs/Inventory Manager'
database=Path(os.environ['LOCALAPPDATA'])/'com.repaircenter.inventorymanager/data/inventory.db'
def digest(path):
    with path.open('rb') as file:return hashlib.file_digest(file,'sha256').hexdigest()
with sqlite3.connect(f'file:{database.as_posix()}?mode=ro',uri=True) as connection:
    tables=['products','stock_entries','sales','sale_lines','fifo_allocations','legacy_archive','migration_info']
    business={'schema':connection.execute('PRAGMA user_version').fetchone()[0],
              'integrity':connection.execute('PRAGMA integrity_check').fetchone()[0],
              'rows':{table:connection.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0] for table in tables}}
business['sha256']=digest(database)
baseline=json.loads(Path(args.baseline).read_text(encoding='utf-8-sig'))
resources=list((root/'src-tauri/resources').rglob('*'))
mismatches=[];count=0
for source in resources:
    if not source.is_file():continue
    count+=1
    relative=source.relative_to(root/'src-tauri/resources')
    target=installed/'resources'/relative
    if not target.is_file() or target.stat().st_size!=source.stat().st_size or digest(target)!=digest(source):mismatches.append(str(relative))
result={'version':args.version,'installed_resource_files':count,'resource_mismatches':mismatches,
        'executable_matches':digest(installed/'inventory-manager.exe')==digest(root/'src-tauri/target/release/inventory-manager.exe'),
        'database_preserved':business==baseline,'database':business,
        'installer_bytes':(root/f'deliverables/Inventory Manager_{args.version}_x64-setup.exe').stat().st_size}
(root/f'tmp/installed-verification-v{args.version}.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
assert not mismatches and result['executable_matches'] and result['database_preserved']
