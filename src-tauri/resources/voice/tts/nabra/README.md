---
base_model: oddadmix/Nabra-82M-v0.1
base_model_relation: quantized
library_name: onnx
pipeline_tag: text-to-speech
language:
  - ar
tags:
  - text-to-speech
  - tts
  - arabic
  - onnx
  - onnxruntime
  - on-device
  - kokoro
license: apache-2.0
---

# Nabra-82M v0.1 — ONNX

ONNX exports of [oddadmix/Nabra-82M-v0.1](https://huggingface.co/oddadmix/Nabra-82M-v0.1),
a Kokoro-82M model fine-tuned for Modern Standard Arabic, packaged for
on-device CPU inference.

All 81.76M parameters are intact. No layers removed, no structural surgery.

| File | Size | Precision |
|---|---:|---|
| `nabra_fp32.onnx` | 325.6 MB | FP32 weights, FP32 compute |

The FP16 variant now lives in [nabra-82m-fp16-onnx](https://huggingface.co/marwanelamami/nabra-82m-fp16-onnx).

## Inputs and outputs

```
input_ids : int64  [1, sequence]   phoneme ids, BOS/EOS wrapped (id 0)
ref_s     : float32[1, 256]        style vector from the voice pack
speed     : float32[1]             1.0 = normal
-> audio  : float32[samples]       24 kHz mono
```

Sequence length is capped at **510** tokens: the voice pack is `[510, 1, 256]`,
so longer inputs have no style row to condition on. Split longer text on
sentence boundaries.

## Usage

```python
import numpy as np, onnxruntime as ort

sess = ort.InferenceSession("nabra_fp32.onnx", providers=["CPUExecutionProvider"])
audio = sess.run(None, {
    "input_ids": np.asarray([input_ids], dtype=np.int64),   # phoneme ids
    "ref_s":     ref_s.astype(np.float32).reshape(1, 256),
    "speed":     np.asarray([1.0], dtype=np.float32),
})[0].squeeze()
```

Phonemes come from espeak-ng (`ar` voice, IPA output), mapped through the
model's `vocab.json`. The graph's stochastic nodes are seeded, so a given
session produces reproducible output.

## Measured performance

POCO F3 (Snapdragon 870), ONNX Runtime 1.26.0 arm64, 4 threads, all cores.
Median of three interleaved rounds with the variant order rotated each round —
the device heats measurably during a run, so ordering matters more than it
looks.

| Variant | Size | RTF | Peak RSS |
|---|---:|---:|---:|
| `nabra_fp32.onnx` | 325.6 MB | 0.628 | 466 MB |
| `nabra_fp16.onnx` | 163.4 MB | 0.622 | 530 MB |

RTF is the ratio of synthesis time to audio duration; below 1.0 is faster than
real time.

### Two things worth knowing before you pick

**FP16 halves the file but uses more RAM.** 530 MB against FP32's 466 MB. The
weights are stored FP16 and cast to FP32 to compute, so the runtime holds both
the FP16 initializers and the FP32 copies. Choose FP16 to reduce download and
storage; do not choose it to reduce memory.

**FP16 is not faster.** 0.622 against 0.628 is inside the run-to-run spread of
a single variant. This is FP16 *storage*, not FP16 arithmetic — ARMv8.0-A has
no native FP16 math, and forcing it there is slower, not quicker.

If peak memory is your constraint, FP32 is the better choice of these two.

## Building the FP16 variant

`onnxconverter_common.float16.convert_float_to_float16` does **not** produce a
loadable model from this graph — it leaves nodes with mismatched FP16/FP32
inputs, because it does not reliably retype tensor-sequence values or recurse
into `Loop` / `If` subgraph bodies, all of which this vocoder uses.

The variant here is built instead by converting each float initializer to FP16
and inserting a `Cast` back to FP32 before its consumer. That loads, and it
keeps compute in FP32 where the hardware is fast.

## Why there is no INT8 or INT4 variant here

Both were built and measured. Neither is worth shipping for this architecture,
and the reasons are specific rather than general — they are about what this
model *is*, not about quantization being bad.

### INT8 dynamic — smallest file, 5.3× slower

`onnxruntime.quantization.quantize_dynamic` with default settings is the
one-liner most people reach for first. On this model it produces the smallest
artifact and the worst latency:

| Variant | Size | RTF | Peak RSS |
|---|---:|---:|---:|
| `nabra_fp32.onnx` | 325.6 MB | 0.628 | 466 MB |
| INT8 dynamic (not published) | 114.7 MB | **3.331** | 285 MB |

Measured 3.322 / 3.331 / 3.350 across three rounds — the tightest numbers in
the whole comparison, so this is not noise.

The cause is `ConvInteger`. ONNX Runtime's integer convolution kernels are a
large regression on ARM64, and this model is **73% Conv by inference time**.
`MatMul` does get a genuine fast integer kernel, but MatMul is only 3.9% of
runtime here, so restricting quantization to it buys almost nothing.

Note the trap: it is the smallest file and it has the lowest peak RSS, so every
selection heuristic except measuring actual latency picks it.

### Weight-only INT8 — works, but buys no speed

Per-output-channel weight-only INT8 (no activation quantization, no calibration
data) does pass quality checks and roughly halves the file. It does not make
inference faster, and the reason is worth knowing before trying it:

**ONNX Runtime constant-folds `DequantizeLinear` on an initializer back to FP32
at session build.** The weights are FP32 in memory from then on and the same
FP32 kernels run. Quantizing weights changes what you *store*, not what the CPU
*does*. Expect a download-size win; do not expect a latency win.

### INT4 — 11× the weight error of INT8

INT4 runs fine on ONNX Runtime 1.26 at opset 21 and packs two values per byte.
The blocker is numerical. Measured on this model's own generator weights:

| Scheme | median rel. error | max | bits/weight |
|---|---:|---:|---:|
| INT8 per-channel | **0.0088** | 0.026 | 8.00 |
| INT4 per-channel | 0.1595 | 0.470 | 4.00 |
| INT4 block=64 | 0.1106 | 0.139 | 4.25 |
| INT4 block=32 | 0.0985 | 0.112 | 4.50 |

Block-wise scaling behaves differently here than it does in LLM quantization,
and the difference explains the result: it cuts the *maximum* error hard
(0.470 → 0.112) while barely moving the median (0.160 → 0.099). Blocking wins
when a few outliers drag the scale for everything sharing it. These convolution
kernels are near-Gaussian — there are no outliers to isolate, so every block
inherits roughly the same range as the whole channel.

Also worth noting: at block=32 the real cost is 4.5 bits per weight once each
block carries its own scale, so the win over INT8 is 1.78×, not 2×.

Post-hoc INT4 would need quantization-aware training to be usable.

### The general lesson

Compute in a neural vocoder is set by **how much work happens at audio rate**,
not by parameter count. On this model the linguistic front-end (PL-BERT plus
the prosody predictor) runs at frame rate and costs 6% of inference; the
vocoder runs at 24 kHz and costs 82%. Quantization reduces stored bytes; it
does not move that boundary. Meaningful latency gains here require a smaller
vocoder, not a smaller number format.

## Limitations

- **Arabic (MSA) only**, single voice. Diacritized input works best.
- **510 token cap** per call.
- Peak memory scales with utterance length, not just weights — roughly 470 MB
  for 3 s of audio and about 1 GB for 17 s on FP32. Synthesize sentence by
  sentence on memory-constrained devices.
- Benchmarked on one device (Snapdragon 870). Low-end SoCs without
  out-of-order cores are substantially slower: on the same phone restricted to
  its four Cortex-A55 cores, RTF is ~2.8.

## License

Apache-2.0, following the base model.


## Quantized variants

| Repo | Precision | Size |
|---|---|---|
| [nabra-82m-fp16-onnx](https://huggingface.co/marwanelamami/nabra-82m-fp16-onnx) | FP16 | 163 MB |
| [nabra-82m-int8-onnx](https://huggingface.co/marwanelamami/nabra-82m-int8-onnx) | INT8 | 83 MB |
| [nabra-82m-int4qat-onnx](https://huggingface.co/marwanelamami/nabra-82m-int4qat-onnx) | INT4-QAT | 57 MB |

The FP32 weights in this repo serve as the fine-tune reference; the quantized repos are drop-in ONNX replacements for on-device deployment.
