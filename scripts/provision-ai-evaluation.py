"""Developer-only, pinned Qwen model evaluation download; never invoked at startup."""
import hashlib, json, pathlib, urllib.request
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'.build'/'ai-model-evaluation';out.mkdir(parents=True,exist_ok=True)
revision='90862c4b9d2787eaed51d12237eafdfe7c5f6077'
name='Qwen3-1.7B-Q8_0.gguf'
url=f'https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/{revision}/{name}'
expected='061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a'
p=out/name
if not p.exists():
    part=p.with_suffix('.download')
    with urllib.request.urlopen(url,timeout=90) as r,part.open('wb') as f:
        while chunk:=r.read(8*1024*1024): f.write(chunk)
    part.replace(p)
actual=hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
if actual!=expected:raise RuntimeError(f'Model checksum mismatch: {actual}')
manifest={'source':url,'revision':revision,'model':name,'sha256':actual,'bytes':p.stat().st_size,'license':'Apache-2.0'}
(out/'SOURCE.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps(manifest),flush=True)
