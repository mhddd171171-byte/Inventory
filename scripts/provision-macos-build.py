"""CI/build-time provisioning. Never imported by the installed application."""
import hashlib
import json
import runpy
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
ai = root / "src-tauri/resources/ai"
record = json.loads((ai / "MODEL-SOURCE.json").read_text(encoding="utf-8"))
path = ai / record["model_file"]
if path.parent.resolve() != ai.resolve():
    raise ValueError("Unsafe model filename.")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


if not path.exists() or digest(path) != record["sha256"]:
    temporary = path.with_suffix(".partial")
    print("Provisioning pinned Qwen model for the offline installer.", flush=True)
    with urllib.request.urlopen(record["source"], timeout=180) as response, temporary.open("wb") as target:
        while chunk := response.read(1024 * 1024):
            target.write(chunk)
    if digest(temporary) != record["sha256"] or temporary.stat().st_size != record["bytes"]:
        raise ValueError("Qwen download checksum/size mismatch.")
    temporary.replace(path)
runpy.run_path(str(root / "scripts/provision-voice.py"), run_name="__main__")
inputs = root / "build-inputs"
inputs.mkdir(exist_ok=True)
llama = inputs / "llama-b11554.tar.gz"
expected = "766cfc5de14ef83c3ec4302a72a8475c366b61e9ceaca516793954336267939d"
url = "https://codeload.github.com/ggml-org/llama.cpp/tar.gz/refs/tags/b11554"
if not llama.exists() or digest(llama) != expected:
    temporary = llama.with_suffix(".partial")
    urllib.request.urlretrieve(url, temporary)
    if digest(temporary) != expected:
        raise ValueError("llama.cpp source checksum mismatch.")
    temporary.replace(llama)
llama.with_suffix(".json").write_text(json.dumps({"version": "b11554", "url": url,
    "sha256": expected, "bytes": llama.stat().st_size}, indent=2), encoding="utf-8")
runpy.run_path(str(root / "scripts/verify-macos-models.py"), run_name="__main__")
