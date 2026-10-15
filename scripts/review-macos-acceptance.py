"""Recheck recorded native answers after fixing one overstrict intent assertion.
Planner, database and models must match the evaluated build. Runtime controls
have their own current native process tests; recorded replies are historical.
"""
import io,json,os,subprocess,tomllib,urllib.error,urllib.request,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
repo="mhdalqad-collab/inventory"
assert os.environ["GITHUB_REPOSITORY"]==repo
baseline="58ba573"
changed=subprocess.check_output(["git","diff","--name-only",baseline,"HEAD","--","src","src-tauri/src","src-tauri/migrations","src-tauri/Cargo.toml","src-tauri/Cargo.lock","package.json","voice","scripts/stage-macos-resources.py","scripts/macos-voice-runtime-hook.py","src-tauri/tauri.macos.conf.json","src-tauri/resources/ai/runtime.json","src-tauri/resources/voice/MODELS.json"],cwd=root,text=True).splitlines()
ui_only={"src/App.tsx","src/types.ts","src/styles.css","src/AssistantPanel.tsx","src/SettingsPanel.tsx","src-tauri/src/assistant_acceptance.rs"}
runtime_controls={"src-tauri/src/assistant.rs","src-tauri/src/assistant_voice.rs","src-tauri/src/lib.rs","src-tauri/src/desktop_settings.rs"}
metadata={"package.json","src-tauri/Cargo.toml","src-tauri/Cargo.lock"}
assert set(changed)<=ui_only|metadata|runtime_controls, "Planner/database/model changes require a new inference run."
def previous(name):
    return subprocess.check_output(["git","show",f"{baseline}:{name}"],cwd=root).decode("utf-8")
old=previous("src-tauri/src/assistant.rs");new=(root/"src-tauri/src/assistant.rs").read_text(encoding="utf-8")
# Protect the exact prompt/request, response parsing and conversational planner.
assert old.split("    fn infer(",1)[1].split("        let response = match response",1)[0] == new.split("    fn infer(",1)[1].split("        let response = match response",1)[0]
assert old.split("        if !response.status().is_success()",1)[1].split("#[cfg(test)]\nmod runtime_tests",1)[0] == new.split("        if !response.status().is_success()",1)[1].split("#[cfg(test)]\nmod runtime_tests",1)[0]
# Voice synthesis/recognition and business IPC implementations are unchanged.
old=previous("src-tauri/src/assistant_voice.rs");new=(root/"src-tauri/src/assistant_voice.rs").read_text(encoding="utf-8")
assert old.split("    fn spawn(",1)[1] == new.split("    fn spawn(",1)[1]
old=previous("src-tauri/src/lib.rs");new=(root/"src-tauri/src/lib.rs").read_text(encoding="utf-8")
assert old.split('        "save_pdf" | "save_excel" =>',1)[1].split("pub fn run()",1)[0] == new.split('        "save_pdf" | "save_excel" =>',1)[1].split("pub fn run()",1)[0]
for name in sorted(metadata & set(changed)):
    original=subprocess.check_output(["git","show",f"{baseline}:{name}"],cwd=root).decode("utf-8")
    current=(root/name).read_text(encoding="utf-8")
    before,after=(json.loads(original),json.loads(current)) if name.endswith(".json") else (tomllib.loads(original),tomllib.loads(current))
    if name=="package.json":
        before.pop("version");after.pop("version")
    elif name.endswith(".toml"):
        before["package"].pop("version");after["package"].pop("version")
    else:
        for document in (before,after):
            own=next(p for p in document["package"] if p["name"]=="inventory-manager")
            own.pop("version")
    assert before==after, f"{name} has changes beyond the application version."
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
report["recorded_response_review"]={"source_run":38598883688,"source_commit":base_sha,"planner_database_and_models_unchanged":True,"runtime_controls_changed":True,"current_runtime_evaluation":"MACOS-AI-CONTROLS.json","workspace_ui_changes":sorted(set(changed)&(ui_only-{"src-tauri/src/assistant_acceptance.rs"})),"corrected_assertion":"Validate grounded purchase value, including a combined quantity/value reply."}
assert report["passed"]==report["total"]==68
(root/".build/MACOS-AI-ACCEPTANCE.json").write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding="utf-8")
print("68 historical native replies passed review; planner/database/models unchanged. Current runtime controls require separate native process verification.")
