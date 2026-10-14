"""Time the actual bilingual planner on Intel macOS before expensive packaging."""
import json, os, platform, re, socket, subprocess, time, urllib.request, uuid
from pathlib import Path
root = Path(__file__).resolve().parents[1]
assert platform.system() == "Darwin" and platform.machine() == "x86_64"
server = root / ".build/llama-macos/bin/llama-server"
if not server.is_file():
    print("No cached native runtime; inference is checked after its build.")
    raise SystemExit(0)
source = (root / "src-tauri/src/assistant_conversation.rs").read_text(encoding="utf-8")
operation = re.search(r'const OPERATION_PROMPT:&str=r#"(.*?)"#;', source, re.S)[1]
format_text = re.search(r'let instructions=format!\("((?:\\.|[^"\\])*)",OPERATION_PROMPT', source)[1]
instructions = json.loads('"' + format_text + '"').replace("{}", operation.replace("Return only JSON with ONE key operation.", "Return only the validated JSON plan."), 1).replace("{{", "{").replace("}}", "}")
operations = re.search(r'"operation":\{"type":"string","enum":(\[[^\]]+\])', source)[1]
schema = {"type":"object","additionalProperties":False,"properties":{
    "operation":{"type":"string","enum":json.loads(operations)},
    "target":{"type":["string","null"]},
    "reference":{"type":"string","enum":["new","selected","other","all","unclear"]},
    "month":{"type":["string","null"]}},"required":["operation","target","reference","month"]}
memory = {"selected":[],"selected_count":0,"listed_candidates":[],"last_search":None,"pending_action":None,"last_action":None,"last_help_topic":None,"last_cr":None}
with socket.socket() as probe:
    probe.bind(("127.0.0.1",0)); port=probe.getsockname()[1]
key=uuid.uuid4().hex
model_name=json.loads((root/"src-tauri/resources/ai/runtime.json").read_text())["model_file"]
args=[str(server),"--model",str(root/"src-tauri/resources/ai"/model_name),"--host","127.0.0.1","--port",str(port),"--api-key",key,"--ctx-size","4096","--parallel","1","--threads","4","--jinja","--reasoning-budget","256","--offline","--no-webui","--no-slots","--no-mmproj","--n-gpu-layers","0","--alias","inventory-intents"]
env={k:v for k,v in os.environ.items() if not k.upper().startswith(("LLAMA_","HF_")) and "PROXY" not in k.upper()}
env.update(VECLIB_MAXIMUM_THREADS="1",HF_HUB_OFFLINE="1",HF_HUB_DISABLE_TELEMETRY="1")
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
log=(root/".build/MACOS-MODEL-PREFLIGHT.log").open("w")
child=subprocess.Popen(args,stdout=subprocess.DEVNULL,stderr=log,env=env)
rows=[]
try:
    started=time.monotonic()
    while True:
        assert child.poll() is None, "Native model exited; see preflight log."
        assert time.monotonic()-started<120, "Native model did not become ready."
        try:
            with opener.open(urllib.request.Request(f"http://127.0.0.1:{port}/health",headers={"Authorization":"Bearer "+key}),timeout=2) as response:
                if response.status==200: break
        except OSError: time.sleep(.3)
    for question in ("What is the total inventory value?", "ما هي قيمة المخزون؟"):
        payload={"model":"inventory-intents","temperature":0,"max_tokens":256,"stream":False,"chat_template_kwargs":{"enable_thinking":False},"response_format":{"type":"json_object","schema":schema},"messages":[{"role":"system","content":instructions},{"role":"user","content":"Memory: "+json.dumps(memory)+"\nRecent conversation: []\nCURRENT message: "+question}]}
        request=urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions",data=json.dumps(payload).encode(),headers={"Authorization":"Bearer "+key,"Content-Type":"application/json"})
        begin=time.monotonic()
        with opener.open(request,timeout=240) as response: result=json.load(response)
        plan=json.loads(result["choices"][0]["message"]["content"])
        row={"question":question,"elapsed_seconds":round(time.monotonic()-begin,2),"plan":plan}
        rows.append(row); print(json.dumps(row,ensure_ascii=False),flush=True)
        assert plan["operation"]=="value", "Native bilingual planner returned the wrong inventory operation."
finally:
    child.terminate()
    try: child.wait(timeout=10)
    except subprocess.TimeoutExpired: child.kill(); child.wait()
    log.close()
    (root/".build/MACOS-MODEL-PREFLIGHT.json").write_text(json.dumps({"rows":rows},indent=2,ensure_ascii=False),encoding="utf-8")
