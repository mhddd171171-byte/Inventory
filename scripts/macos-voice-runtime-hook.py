"""Resolve bundled pronunciation tables after the Intel app is relocated."""
import os
import sys
from pathlib import Path

if sys.platform == "darwin" and getattr(sys, "frozen", False):
    data = Path(sys.executable).resolve().parent.parent / "espeak-ng-data"
    if not (data / "phontab").is_file():
        raise RuntimeError("Bundled pronunciation tables are missing; repair the Mac installation.")
    os.environ["ESPEAK_DATA_PATH"] = str(data)
    import piper.phonemize_espeak as phonemizer
    phonemizer.ESPEAK_DATA_DIR = data
    phonemizer.EspeakPhonemizer.__init__.__defaults__ = (data,)
