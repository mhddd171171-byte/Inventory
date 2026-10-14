"""Verify the shared offline models before a native Mac build. No downloads."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QWEN_SHA256 = "2fde00ce69dd4899c70d020845e2638353015bba0fdf161b3eb965f2bca4464e"


def verified_models():
    source = ROOT / "src-tauri/resources"
    profile = json.loads((source / "ai/runtime.json").read_text(encoding="utf-8"))
    name = profile["model_file"]
    if Path(name).name != name or name != "Qwen3-4B-Instruct-2507-Q4_K_M.gguf":
        raise ValueError("Unexpected model. Re-evaluate a replacement before packaging it.")
    expected = [(source / "ai" / name, QWEN_SHA256, 2497280736)]
    for item in json.loads((source / "voice/MODELS.json").read_text(encoding="utf-8")):
        path = (source / "voice" / item["path"]).resolve()
        if not path.is_relative_to((source / "voice").resolve()):
            raise ValueError("Voice model path escapes resources.")
        expected.append((path, item["sha256"], item["bytes"]))
    records = []
    for path, digest, size in expected:
        if not path.is_file() or path.stat().st_size != size:
            raise ValueError(f"Missing or incomplete offline model: {path}")
        with path.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != digest:
            raise ValueError(f"Model checksum mismatch: {path}")
        records.append({"path": path.relative_to(ROOT).as_posix(), "bytes": size, "sha256": actual})
    return records


if __name__ == "__main__":
    records = verified_models()
    print(f"Verified {len(records)} shared model files ({sum(r['bytes'] for r in records):,} bytes).")
