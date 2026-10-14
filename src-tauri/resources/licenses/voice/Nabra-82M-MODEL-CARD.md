---
license: apache-2.0
language:
- ar
pipeline_tag: text-to-speech
library_name: kokoro
base_model: hexgrad/Kokoro-82M
tags:
- text-to-speech
- tts
- arabic
- ar
- kokoro
- styletts2
- speech-synthesis
- on-device
---

# Nabra-82M — Arabic Text-to-Speech

**Nabra** is an 82M-parameter neural text-to-speech model for **Modern Standard
Arabic (MSA)**. It is a fine-tune of [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)
(a StyleTTS2 / ISTFTNet architecture), adapted to Arabic phonetics with a
dedicated pharyngeal-consonant vocabulary and an Arabic grapheme-to-phoneme
front-end. It ships a single, natural female voice — `af_msa`.

> ⚠️ **Nabra expects diacritized (tashkeel'd) input.** Arabic drops short vowels
> in normal writing; without them the phonemizer guesses badly. Run text through
> a diacritizer first (see [G2P pipeline](#g2p-pipeline)).

## Highlights

- 🗣️ **Natural MSA speech** at 24 kHz from a compact 82M model.
- 🔤 **Arabic-aware phonemes** — the pharyngeal fricatives ع (`ʕ`) and ح (`ħ`)
  get dedicated embedding slots (Kokoro vocab ids 7 & 8), preserving the ع/ء and
  ح/ه contrasts that generic phonemizers collapse.
- 📱 **Runs on-device** — an MLX conversion ([`oddadmix/Nabra-82M-MLX`](https://huggingface.co/oddadmix/Nabra-82M-MLX))
  runs fully offline on Apple Silicon / iOS via [kokoro-ios](https://github.com/Oddadmix/kokoro-ios).
- ⚡ **Real-time factor < 1** on CPU for typical sentences.

## Intended Use

- Arabic voice assistants, screen readers, and accessibility tools
- Audiobook / e-learning narration in MSA
- Voice output for on-device Arabic LLM pipelines (STT → LLM → diacritize → **Nabra**)

**Out of scope:** dialectal Arabic (trained on MSA), singing, non-Arabic text,
and speaker cloning (single fixed voice).

## Usage

Nabra runs through the [Kokoro](https://github.com/hexgrad/kokoro) pipeline with
an Arabic front-end: **normalize → diacritize (camel-tools) → phonemize
(espeak-ng) → synthesize.**

```bash
# Install Kokoro from the Nabra fork — it carries the Arabic config/vocab
pip install "kokoro @ git+https://github.com/Oddadmix/kokoro.git@main"
pip install torch soundfile huggingface_hub misaki camel-tools
# one-time: fetch the MSA diacritizer model (not shipped in the pip package)
camel_data -i disambig-mle-calima-msa-r13
# grab the Arabic G2P front-end
wget https://gist.githubusercontent.com/Oddadmix/dc699f7942a9516ce29d4842c7aed756/raw/827b541c892a862f9ef3b44006a6e27b100d1bdd/arabic_g2p.py
```

```python
import numpy as np, torch, soundfile as sf
from huggingface_hub import hf_hub_download, list_repo_files
from arabic_g2p import ArabicG2P, EXTRA_SYMBOLS, clean_phonemes, normalize_text
from kokoro import KModel, KPipeline
from kokoro import pipeline as kpipeline_mod

REPO_ID = "oddadmix/Nabra-82M-v0.1"
files = list_repo_files(REPO_ID)
model_file = next(f for f in files if f.endswith(".pth"))   # fine-tuned weights
voice_file = next(f for f in files if f.endswith(".pt"))    # af_msa voicepack
config     = hf_hub_download(REPO_ID, "config.json")
model_path = hf_hub_download(REPO_ID, model_file)
voice_path = hf_hub_download(REPO_ID, voice_file)

# Load OUR fine-tuned weights (config + model given → base model is never fetched).
# disable_complex=True uses the real-valued STFT (robust on all GPUs).
kmodel = KModel(repo_id=REPO_ID, config=config, model=model_path,
                disable_complex=True).eval()
kmodel.vocab.update(EXTRA_SYMBOLS)                          # ʕ→7, ħ→8

# Route the Kokoro pipeline through espeak-ng Arabic + phoneme cleanup
kpipeline_mod.LANG_CODES.setdefault("ar", "ar")
pipeline = KPipeline(lang_code="ar", repo_id=REPO_ID, model=kmodel)
_orig_g2p = pipeline.g2p
pipeline.g2p = lambda t: (clean_phonemes(_orig_g2p(t)[0]), _orig_g2p(t)[1])

voice = torch.load(voice_path, map_location="cpu", weights_only=True)
g2p = ArabicG2P(diacritize=True)                           # camel-tools MLE

# ── Synthesize ────────────────────────────────────────────────────────────────
text = "مرحبا بك في نبرة"                                  # raw MSA (tashkeel optional)
text_norm, _ = normalize_text(text)
diac = g2p.diacritize(text_norm)                           # → مَرْحَبًا بِكَ فِي نَبْرَة

audios = [audio for _, _, audio in pipeline(diac, voice=voice, speed=1.0)]
wav = np.concatenate([a.detach().cpu().numpy() for a in audios]).astype(np.float32)
sf.write("nabra.wav", wav, 24000)
```

**Already diacritized?** Skip step 2 — pass your tashkeel'd text straight to the
pipeline (`pipeline(text, voice=voice)`), or build `ArabicG2P(diacritize=False)`.

- 🎮 **Try it in the browser:** [Nabra-82M Demo Space](https://huggingface.co/spaces/oddadmix/Nabra-82M-Demo)
- 📱 **On-device (iOS / MLX):** [`oddadmix/Nabra-82M-MLX`](https://huggingface.co/oddadmix/Nabra-82M-MLX) + [kokoro-ios](https://github.com/Oddadmix/kokoro-ios) (bundles neural diacritization, no camel-tools needed)

## G2P Pipeline

Text is converted to phonemes in four stages so training and inference use an
identical symbol set:

1. **Normalize** — strip citation markers and Latin-script loanwords, tidy whitespace.
2. **Diacritize** — restore MSA short vowels with camel-tools MLE (`calima-msa-r13`).
   Skipped if the text is already diacritized. For a fully neural, on-device
   alternative, use [CATT](https://github.com/abjadai/catt).
3. **Phonemize** — espeak-ng Arabic (`ar`) → IPA.
4. **Clean** — strip espeak's mid-word syllable dots (which would inject phantom
   pauses) and pharyngealization/bracket markers; keep `ʕ`/`ħ`.

## Model Details

| | |
|---|---|
| **Base model** | [hexgrad/Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) |
| **Architecture** | StyleTTS2 (PL-BERT text encoder + prosody predictor + ISTFTNet decoder) |
| **Parameters** | ~82M |
| **Language** | Modern Standard Arabic (`ar`) |
| **Sample rate** | 24 kHz, mono |
| **Voices** | `af_msa` (female) |
| **Vocab** | 178-token Kokoro table + `ʕ`→7, `ħ`→8 |
| **Training** | Fine-tuned with a patched StyleTTS2 |

## Files & Formats

- **This repo** (`Nabra-82M-v0.1`, PyTorch/KModel): `kokoro_arabic.pth`, `af_msa.pt`, `config.json`
- **MLX** ([`Nabra-82M-MLX`](https://huggingface.co/oddadmix/Nabra-82M-MLX)): `kokoro-v1_0.safetensors`, `af_msa.safetensors`, `config.json`

## Limitations & Notes

- **Requires diacritized input** for correct pronunciation.
- **MSA only** — not trained for Egyptian or other dialects.
- **Single voice** (`af_msa`); additional voices need more voicepacks.

## Related

- 🧩 [kokoro-ios](https://github.com/Oddadmix/kokoro-ios) — MLX Swift inference (TTS + on-device Arabic diacritization)
- 🍴 [Oddadmix/kokoro](https://github.com/Oddadmix/kokoro) — Kokoro fork with the Arabic config/vocab
- 📜 [kikiri-tts](https://github.com/semidark/kikiri-tts) — upstream Kokoro/StyleTTS2 fine-tuning recipe
- 🔤 [CATT](https://github.com/abjadai/catt) — Arabic diacritization (Apache-2.0)

## Citation

```bibtex
@misc{nabra2026,
  title  = {Nabra-82M: Modern Standard Arabic Text-to-Speech},
  author = {oddadmix},
  year   = {2026},
  howpublished = {\url{https://huggingface.co/oddadmix/Nabra-82M-v0.1}}
}
```

## License & Attribution

Released under **Apache-2.0**, following [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)
(Apache-2.0) and [StyleTTS2](https://github.com/yl4579/StyleTTS2) (MIT). Arabic
diacritization in the pipeline uses [camel-tools](https://github.com/CAMeL-Lab/camel_tools)
and/or [CATT](https://github.com/abjadai/catt) (Apache-2.0).

The fine-tuning recipe is built on [**kikiri-tts**](https://github.com/semidark/kikiri-tts)
by [@semidark](https://github.com/semidark) — the upstream Kokoro/StyleTTS2 training
workflow this Arabic adaptation extends.
