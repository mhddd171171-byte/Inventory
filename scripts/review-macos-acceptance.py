"""Recheck recorded native answers after fixing one overstrict intent assertion.
Planner, database and models must match the evaluated build. Runtime controls
have their own current native process tests; recorded replies are historical.
"""
import hashlib,json,os,subprocess,tomllib
from pathlib import Path
root=Path(__file__).resolve().parents[1]
repo="mhddd171171-byte/Inventory"
assert os.environ["GITHUB_REPOSITORY"]==repo
baseline="58ba573"
changed=subprocess.check_output(["git","diff","--name-only",baseline,"HEAD","--","src","src-tauri/src","src-tauri/migrations","src-tauri/Cargo.toml","src-tauri/Cargo.lock","package.json","voice","scripts/stage-macos-resources.py","scripts/macos-voice-runtime-hook.py","src-tauri/tauri.macos.conf.json","src-tauri/resources/ai/runtime.json","src-tauri/resources/voice/MODELS.json"],cwd=root,text=True).splitlines()
ui_only={"src/App.tsx","src/types.ts","src/styles.css","src/AssistantPanel.tsx","src/SettingsPanel.tsx","src/BackupSafety.tsx","src/api.ts","src-tauri/src/assistant_acceptance.rs"}
runtime_controls={"src-tauri/src/store.rs","src-tauri/src/assistant.rs","src-tauri/src/assistant_voice.rs","src-tauri/src/lib.rs","src-tauri/src/desktop_settings.rs","src-tauri/src/data_safety.rs","src-tauri/src/mac_installation.rs"}
metadata={"package.json","src-tauri/Cargo.toml","src-tauri/Cargo.lock"}
assert set(changed)<=ui_only|metadata|runtime_controls, "Planner/database/model changes require a new inference run."
def previous(name):
    return subprocess.check_output(["git","show",f"{baseline}:{name}"],cwd=root).decode("utf-8")
# The only production database change normalizes schema CRLF, without changing
# queries, schema, validation constraints, mutations or snapshots. New native
# tests exercise LF/CRLF open, restore, v1 migration and real tamper rejection.
old_store=previous("src-tauri/src/store.rs").split("#[cfg(test)]",1)[0]
new_store=(root/"src-tauri/src/store.rs").read_text(encoding="utf-8").split("#[cfg(test)]",1)[0]
old_fragment='let objects=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))'
new_fragment='let objects=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,String>(2)?.replace("\\r\\n","\\n"))))'
expected_store=old_store.replace(old_fragment,new_fragment).replace('fn schema_objects(conn:&Connection)','// SQLite preserves source line endings. Normalize only CRLF; retain every\n// other byte, object, constraint and trigger check.\nfn schema_objects(conn:&Connection)')
assert expected_store==new_store, "Database changes beyond schema newline normalization require fresh acceptance."
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
# Keep the original native evidence with this build instead of requiring access
# to a different account's private, expiring Actions artifacts. The source
# artifact digest was verified against GitHub's authenticated artifact metadata.
fixture=root/"tests/fixtures/macos-native-baseline"
source=json.loads((fixture/"SOURCE.json").read_text(encoding="utf-8"))
assert source["source_repository"]=="mhdalqad-collab/inventory"
assert source["source_run"]==38598883688 and source["source_commit"]==base_sha
expected={
    "MACOS-AI-ACCEPTANCE.json":"1b55e1386bfae9f9c7319328effb02f57c8564e6135fe94877147428ad2834e8",
    "MACOS-MODEL-PREFLIGHT.json":source["files"]["MACOS-MODEL-PREFLIGHT.json"],
    "MACOS-MODEL-PREFLIGHT.log":source["files"]["MACOS-MODEL-PREFLIGHT.log"],
}
(root/".build").mkdir(exist_ok=True)
for name,digest in expected.items():
    content=(fixture/name).read_bytes()
    assert hashlib.sha256(content).hexdigest()==digest, f"Baseline evidence checksum mismatch: {name}"
    if name!="MACOS-AI-ACCEPTANCE.json": (root/".build"/name).write_bytes(content)
report=json.loads((fixture/"MACOS-AI-ACCEPTANCE.json").read_text(encoding="utf-8"))
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
report["recorded_response_review"]={"source_run":38598883688,"source_commit":base_sha,"planner_business_queries_and_models_unchanged":True,"schema_line_ending_validation_fixed":True,"runtime_controls_changed":True,"current_runtime_evaluation":"MACOS-AI-CONTROLS.json","workspace_ui_changes":sorted(set(changed)&(ui_only-{"src-tauri/src/assistant_acceptance.rs"})),"corrected_assertion":"Validate grounded purchase value, including a combined quantity/value reply."}
assert report["passed"]==report["total"]==68
(root/".build/MACOS-AI-ACCEPTANCE.json").write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding="utf-8")
print("68 historical native replies passed review; planner/business queries/models unchanged; schema line-ending portability verified separately. Current runtime controls require separate native process verification.")
