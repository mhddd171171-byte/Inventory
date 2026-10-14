"""Build-only optional-import patch. Microphone samples already are float32 PCM,
so the packaged worker needs no media-file decoders or FFmpeg/PyAV libraries.
The original decoder remains available in the development environment.
"""
from importlib.util import find_spec
from pathlib import Path

path = Path(find_spec("faster_whisper").origin).parent / "audio.py"
text = path.read_text(encoding="utf-8")
if "import av\n" in text and "except ImportError:" not in text:
    text = text.replace("import av\n", "# Al Mohtarif modification: PyAV is optional for in-memory PCM inference.\ntry:\n    import av\nexcept ImportError:\n    av = None\n", 1)
    text = text.replace("    resampler = av.audio.resampler.AudioResampler(", "    if av is None:\n        raise RuntimeError('This offline worker accepts microphone PCM, not encoded media files.')\n    resampler = av.audio.resampler.AudioResampler(", 1)
    path.write_text(text, encoding="utf-8")
print("Applied optional media-decoder import for PCM-only worker.")
