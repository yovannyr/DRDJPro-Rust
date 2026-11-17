# A0 CPU backend experiment

This workspace member has **no application dependency or UI/audio integration**.
Default features load/link neither runtime. `ort` and `tract` are independent,
opt-in adapters behind `InferenceBackend`; both accept only the synthetic fixture's
eight input floats and return its twelve output floats. They allocate and may
block. Construct and execute them on an owned non-audio worker. This is not yet
the A1 inference service, model registry, scheduler or persistent cache.

The generated Conv→Transpose→GRU graph is 489 bytes, untrained and contains no
third-party model weights. `tools/ml_fixture.py` creates it outside the source
tree using Python stdlib and ONNX IR8/opset13. Checked-in `tests/reference.json`
contains three scalar reference cases, model SHA256 and 1e-5 absolute tolerance.
Every enabled backend is tested at 1/2/4 threads, twice, to detect retained GRU
state. This validates tensor/operator plumbing, not beat/downbeat quality,
real-music speed, large-model memory, STFT or transformer compatibility.

## Reproduce

From the repository root, set `CARGO_BUILD_JOBS=1` and an existing shared
`CARGO_TARGET_DIR`. Select one backend at a time to limit artifacts.

1. `python tools/ml_fixture.py --model <absolute-target-path>/conv-gru.onnx`
2. Set `DRDJ_ML_FIXTURE` to that absolute path. ORT only: explicitly run
   `python tools/ml_spike.py fetch-ort --destination <target-path>/ort-runtime`
   and set `DRDJ_ORT_LIBRARY` to the resulting absolute `onnxruntime.dll` path.
   Fetch currently supports the pinned Windows x64 CPU package only.
3. `cargo test -p drdj-ml --features ort --locked` (or `tract`). After the required
   Rust sources are cached, repeat with `--offline` to verify reproducibility.
4. `cargo clippy -p drdj-ml --features ort --all-targets --locked -- -D warnings`
5. `python -m unittest discover -s tools -p "test_ml_*.py"`

For optimized measurements only:

```
cargo build -p drdj-ml --example probe --features ort --profile ml-spike --locked --offline
python tools/ml_spike.py probe --binary <target>/ml-spike/examples/probe.exe --backend ort --model <model> --runtime <dll> --output <target>/ort-measurements.json
```

Use `--features tract`, `--backend tract` and omit `--runtime` for the second
candidate. The tool starts a fresh process per thread configuration, records
first/repeated model loading and ten timing batches, and measures Windows peak
working set before acknowledging child exit. A batch is 75 independent eight-frame
chunks at an **artificial** 10Hz: the resulting synthetic minute is not an audio
import RTF. Non-Windows peak memory is explicitly unmeasured (`null`).

ORT requires ownership of its process-wide initialization; don't combine this
spike with direct ORT clients. Canonical paths are serialized and a different
library is rejected. The runtime archive is size/SHA256 pinned; only the CPU DLLs
and license notices are retained. Cargo's automatic binary download/link features
are disabled. Graph inputs must be trusted local fixtures, not arbitrary downloaded
ONNX or external-weight graphs. General model verification belongs to A1.

Decision, measured numbers and limits: [ML-BACKEND.md](../../docs/adr/ML-BACKEND.md).
