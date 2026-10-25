"""Reuse verified 2.0.9 inference evidence only for this strictly UI-only update."""
import hashlib,json,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1]
fixture=root/'tests/fixtures/macos-2.0.9'
source=json.loads((fixture/'SOURCE.json').read_text(encoding='utf-8'))
baseline='84d46f5b7a05a7cbf73815a24cb41be5be38631e'
assert source['commit']==baseline and source['run']==40199645952
changed=set(subprocess.check_output(['git','diff','--name-only',baseline,'HEAD'],cwd=root,text=True).splitlines())
metadata={'package.json','package-lock.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/installer/windows.iss','scripts/build-windows.ps1'}
allowed=metadata|{'.gitattributes','src/StockForms.tsx','src/styles.css','tests/ui-flow.mjs','.github/workflows/macos-intel.yml','scripts/build-macos-intel.sh','scripts/publish-macos-release.py','scripts/review-ui-only-ai-evidence.py','tests/fixtures/macos-2.0.9/SOURCE.json','tests/fixtures/macos-2.0.9/MACOS-AI-ACCEPTANCE.json'}
assert changed<=allowed, f'Fresh AI evaluation required for changes outside this UI release: {sorted(changed-allowed)}'
for name in metadata&changed:
 before=subprocess.check_output(['git','show',f'{baseline}:{name}'],cwd=root).decode('utf-8').replace('\r\n','\n')
 after=(root/name).read_text(encoding='utf-8').replace('2.0.10','2.0.9')
 assert before==after, f'{name} changed beyond release version; fresh evaluation required.'
raw=(fixture/'MACOS-AI-ACCEPTANCE.json').read_bytes()
assert hashlib.sha256(raw).hexdigest()==source['sha256']
report=json.loads(raw)
assert report['passed']==report['total']==68
assert report['model']=='Qwen3-4B-Instruct-2507-Q4_K_M.gguf'
assert all(row['passed'] for row in report['rows'])
report['recorded_response_review']={'source_repository':'mhddd171171-byte/Inventory','source_run':source['run'],'source_commit':baseline,'source_sha256':source['sha256'],'reason':'Stock Out picker and theme only. AI prompts, inference code, business queries, database schema, voice code, model manifests and pinned dependencies are unchanged. Current native/backend/UI, voice synthesis, installed startup and package checks run separately.','fresh_inference_run':False}
(root/'.build/MACOS-AI-ACCEPTANCE.json').write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding='utf-8')
print('Reviewed 68 verified 2.0.9 native answers; strict UI-only source guard passed. Fresh package/runtime checks are separate.')
