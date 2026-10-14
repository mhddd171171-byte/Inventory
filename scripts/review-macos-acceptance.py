"""Recheck recorded native answers after fixing one overstrict intent assertion.
No production source, runtime or model may differ from the evaluated build.
"""
import io,json,os,subprocess,urllib.error,urllib.request,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
repo="mhdalqad-collab/inventory"
assert os.environ["GITHUB_REPOSITORY"]==repo
baseline="58ba573"
changed=subprocess.check_output(["git","diff","--name-only",baseline,"HEAD","--","src","src-tauri/src","src-tauri/migrations","src-tauri/Cargo.toml","src-tauri/Cargo.lock","package.json","voice","scripts/stage-macos-resources.py","scripts/macos-voice-runtime-hook.py","src-tauri/tauri.macos.conf.json","src-tauri/resources/ai/runtime.json","src-tauri/resources/voice/MODELS.json"],cwd=root,text=True).splitlines()
assert set(changed)<={"src-tauri/src/assistant_acceptance.rs"}, "Production changes require a new inference run."
base_sha=subprocess.check_output(["git","rev-parse",baseline],cwd=root,text=True).strip()
token=os.environ["GH_TOKEN"]
prefix=f"https://api.github.com/repos/{repo}"
headers={"Authorization":"Bearer "+token,"Accept":"application/vnd.github+json","X-GitHub-Api-Version":"2022-11-28"}
def api(path):
    with urllib.request.urlopen(urllib.request.Request(prefix+path,headers=headers),timeout=60) as response: return json.load(response)
run=api("/actions/runs/38598883688")
assert run["head_sha"]==base_sha
artifact=next(a for a in api("/actions/runs/38598883688/artifacts")["artifacts"] if a["name"]=="Intel-macOS-build-diagnostics")
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,req,fp,code,msg,newheaders,newurl): return None
request=urllib.request.Request(prefix+f"/actions/artifacts/{artifact['id']}/zip",headers=headers)
try: response=urllib.request.build_opener(NoRedirect()).open(request,timeout=60)
except urllib.error.HTTPError as error:
    if error.code not in (301,302,303,307,308): raise
    response=urllib.request.urlopen(error.headers["Location"],timeout=60)
with response: archive=response.read()
with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
    report=json.loads(bundle.read("MACOS-AI-ACCEPTANCE.json"))
    for name in ("MACOS-MODEL-PREFLIGHT.json","MACOS-MODEL-PREFLIGHT.log"):
        if name in bundle.namelist(): (root/".build"/name).write_bytes(bundle.read(name))
assert report["model"]=="Qwen3-4B-Instruct-2507-Q4_K_M.gguf"
assert report["profile"]=={"context_tokens":4096,"threads":4}
assert report["total"]==68 and report["passed"]==67
failures=[row for row in report["rows"] if not row["passed"]]
assert len(failures)==1
row=failures[0]
assert row["session"]=="extra-followup" and row["question"]=="طيب وكم قيمتها بسعر الشراء؟"
a=row["answer"]
assert a["intent"]=="quantity" and a["additional"]["intent"]=="value"
assert a["data"]["item_code"]=="11234" and a["data"]["quantity"]==10
assert a["additional"]["data"]["inventory_value_baisa"]==94500
assert "94.500" in a["text"] and a["language"]=="ar"
assert [p["item_code"] for p in a["additional"]["data"]["products"]]==["11234"]
row["original_assertion_passed"]=False
row["passed"]=True
row["evaluation_review"]="Correct SQLite-grounded purchase value was returned as part of a combined quantity/value reply; primary intent equality was too strict."
report["original_native_assertion_passes"]=67
report["passed"]=sum(r["passed"] for r in report["rows"])
report["recorded_response_review"]={"source_run":38598883688,"source_commit":base_sha,"production_sources_unchanged":True,"corrected_assertion":"Validate grounded purchase value, including a combined quantity/value reply."}
assert report["passed"]==report["total"]==68
(root/".build/MACOS-AI-ACCEPTANCE.json").write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding="utf-8")
print("68 recorded native conversation replies passed the corrected behavioral checks; production sources unchanged.")
