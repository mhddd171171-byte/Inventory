---
license: apache-2.0
language: ar
library_name: onnx
tags:
- arabic
- diacritization
- tashkeel
- onnx
- transformer
---

# CATT-EO — Arabic diacritizer (stitched single ONNX)

Single-file ONNX of the **encoder-only [CATT](https://github.com/abjadai/catt)** model
(Abjad AI, Apache-2.0). CATT-EO is non-autoregressive — a transformer encoder followed by
a linear classifier head — so [TigreGotico](https://huggingface.co/TigreGotico)
**stitched the two stages into one ONNX** with `onnx.compose`. The stitched graph is
**byte-identical** (max|Δ| = 0.0) to the original two-stage encoder→head pipeline.

| file | precision | size |
|------|-----------|-----:|
| `catt_eo.onnx` | fp32 | 78 MB |
| `catt_eo.int8.onnx` | dynamic int8 | 21.6 MB |

**I/O.** Inputs `src` (int64, `[batch, seq]` Buckwalter ids) and `src_mask`
(bool, `[batch, 1, seq, seq]`). Output `[batch, seq, 18]` tag logits. Tokenizer:
`tashkeel_tokenizer_onnx.py` (+ `bw2ar.py`, `utils.py`) — Buckwalter encoding and the
18-tag tashkeel scheme. See `catt_models_onnx.py` for an end-to-end example.

**Benchmark.** ~4.27 % DER on the broad
[`TigreGotico/arabic_diacritized_text`](https://huggingface.co/datasets/TigreGotico/arabic_diacritized_text)
test (much lower on CATT's own narrow benchmark — distribution-dependent). The int8 file
is dynamic-quantized; expect small divergence from fp32.

Part of the
[Arabic Diacritizers / Tashkeel collection](https://huggingface.co/collections/TigreGotico/arabic-diacritizers-tashkeel-6a247318559bcc49e128aa5f).
Original CATT © Abjad AI (Apache-2.0); this is a stitched re-export with attribution.


## Encoder-decoder variant (CATT-ED)

`catt_ed_encoder.onnx` + `catt_ed_decoder.onnx` (+ `.int8`) are the **autoregressive** encoder-decoder model — the decoder runs once per output position (causal mask), so it is two ONNX (not stitchable) and much slower than CATT-EO. In `text2tashkeel` it is the `catt-ed` / `catt-ed-int8` model.
