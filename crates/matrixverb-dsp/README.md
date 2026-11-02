# MatrixVerb DSP

Full stereo MatrixVerb signal-path port from Airwindows revision
`7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a`, with attribution and
[upstream MIT license](../../poc/matrix/LICENSE.airwindows) retained.
The separate host plugin is in [plugins/matrixverb](../../plugins/matrixverb).
Ultralight3's plugin and identities are not changed by this addition.

## Processing contracts

- `ProcessingParameters` carries all seven original normalized controls and
  defaults. `Tuning::new()` rejects nonfinite/out-of-range controls and rates
  outside 22.05–384 kHz before processing changes any state.
- `Processor::process::<APPROXIMATE, ACCELERATED>()` runs one complete stereo
  backend with `f32` I/O and `f64` internals, including upstream input noise
  and 32-bit dither. `process_f64()` retains the upstream double path's RNG
  advancement without output dither.
- `DspCore` is the plugin-facing dual processor. Both histories stay live.
  Original/approximate changes crossfade over 50 ms; acceleration changes
  immediately, without resets. Buffers are distinct borrowed input/output
  slices with equal lengths asserted.
- Delay storage is allocated once during construction, before the audio
  callback. Processing, automation, and reset do not allocate. The host
  wrapper uses fixed-size stack-owned staging buffers.
- Generic math switches compile to static scalar or SIMD/FMA kernels.
  Acceleration off is scalar/unfused. Acceleration on uses two-wide `f64`
  vectors across stereo channels for filters, allpasses, modulated delay
  interpolation, feedback matrices and mixing, plus vector polynomial
  evaluation in approximate mode. Remaining selection and wrapping use masks.
  The original path deliberately retains upstream serial sum association
  and `f64` libm sine/arcsine for fidelity. Approximate math uses polynomial
  replacements and balanced reductions; do not call it bit-exact. Sine uses
  symmetry folding to [-pi/2, pi/2] and a degree-15 Estrin polynomial;
  its independent coefficient groups shorten scalar/vector dependency chains.
- SIMD types come from `wide`; fused intrinsics are isolated in the shared
  [CPU backend](../../poc/matrix/src/acceleration.rs). Apple Silicon uses
  NEON/FMA. x86_64 uses SSE2 and runtime-detected FMA, falling back to scalar
  fused lane operations on CPUs without FMA. Both plugin initialization and
  benchmarks explicitly report the selected backend. ARM and Linux x86_64
  execution have been validated locally; Linux DAW testing remains pending.
- Coefficient updates occur outside the sample loop and retain the reference's
  tangent/power mapping. Room size changes preserve upstream ring histories,
  even when a cursor temporarily lies beyond the new logical delay.
- Denormal input noise and deterministic dither follow the pinned processor.
  The plugin wrapper relies on NIH-plug FTZ handling; standalone callers must
  provide a compatible floating-point environment for silent-tail timing.
- Startup phases are specified in [state.rs](src/dsp/state.rs), together with
  the two RNG seeds, rather than reproducing the upstream constructor's
  platform-dependent random initialization.

The original-order Householder kernel reuses the already validated
[shared DSP implementation](../../poc/matrix/src/dsp/core.rs).

## Validation

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo test --locked --release --package oxidation-matrixverb-dsp --package oxidation-matrixverb
sh scripts/matrixverb-reference.sh
```

After bundling both plugins, `sh scripts/check-simd.sh` disassembles the
actual CLAP binaries on macOS/Linux and fails unless they contain packed
FMA instructions. This verifies code generation, not a performance benefit.

The first command requires only Rust and runs allocation/reset, block
invariance, all-control effects, mode distinctness, reversible crossfade,
sample-rate validation, ten-second tail/error-budget, and scalar/vector
polynomial tests. All four integrated modes are compared with the
corresponding statically specialized single processor including dither.
All sixteen starting/ending mode pairs are checked against continuously
running single processors during live transitions. SIMD outputs are also
compared with an independent scalar fused path under asymmetric stereo
input and live room/modulation changes.
The complete C++ comparison is an explicitly ignored integration test in
that command; the second command runs it.

The reference script requires `curl` and a C++17 compiler (`CXX` can select
it), downloads the pinned
[unmodified processor](https://github.com/airwindows/airwindows/blob/7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a/plugins/WinVST/MatrixVerb/MatrixVerbProc.cpp)
into ignored build storage, and compiles it with
[a DSP-only state shim](tests/reference/MatrixVerb.h).
No VST2 SDK is needed. The shim uses the same fixed phases and RNG seeds as
Rust; all processing expressions come from the upstream file. The reference
is test tooling only, not a runtime or normal-build dependency.

The test compares every stereo output sample for float and double callbacks
over 32,768-frame renders, ten scenarios, and 22.05/44.1/48/96/192/384 kHz:
defaults; parameter endpoints; minimum room/maximum modulation; positive
and negative crossmodulation; damping; dry-only; silence; and live
room/modulation/filter changes at frames 8,192 and 16,384. This is
7,864,320 output samples per mode, or 31,457,280 across all four modes.
Acceptance bounds are peak/RMS absolute error `2e-6 / 1e-7` for `f32`;
`f64` original/scalar uses `1e-10 / 1e-12`, and the other modes use
`1e-5 / 1e-6`, allowing approximation and platform libm differences.
M4 Pro peak errors, in O/S, A/S, O/H, A/H order:

| Precision | O/S | A/S | O/H | A/H |
| --- | ---: | ---: | ---: | ---: |
| f32 | 0 | 2.33e-10 | 0 | 2.33e-10 |
| f64 | 0 | 3.11e-11 | 6.60e-17 | 3.11e-11 |

O/A means original/approximate math; S/H means scalar/CPU SIMD-FMA.
This is not a universal bit-identity proof.

Polynomial tests cover 100,001 points: sine error below `6e-10` for phases
between -1000 and 1000 radians, and arcsine error below `1.1e-8` across
[-1, 1]. Ten-second impulse tails at three Flavor settings, maximum Speed
and Vibrato, and minimum Damping require finite outputs below magnitude 4
and peak/RMS difference below `1e-5 / 1e-6` for all non-original modes.

## Measurement

The reference script also benchmarks the complete single C++ processor and
all four single Rust backends, plus all four integrated dual-backend modes.
Standalone Rust benchmarking (no C++ needed):

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --locked --release --package oxidation-matrixverb-dsp --bin matrixverb-bench
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --locked --release --package oxidation-matrixverb-dsp --bin matrixverb-bench -- reverse
```

Times are ns per **stereo frame**, not per channel and not the earlier
matrix-only ns/transform. Every mode processes the same 256-frame stereo
input with default controls at 44.1 kHz, warms up for 131,072 frames, then
reports the median/min/max of eleven 131,072-frame rounds. State persists
between rounds; every mode starts from identical initialization and input
before its own warmup.
C++ uses `-O3 -fno-fast-math -ffp-contract=off` and disables vectorization.
Rust uses the workspace release profile and disables auto-vectorization.
Explicit SIMD/FMA is used only in accelerated modes. Compiler settings in
the workspace preserve scalar math in the nonaccelerated modes.
The portable builds use default CPU targets; the Linux CPU-native comparison
adds `-march=native` for C++ and `-C target-cpu=native` for Rust. The C++
processor is compiled as a separate translation unit without LTO; Rust uses
workspace fat LTO.
C++ recomputes coefficients each block; the single-backend Rust benchmark
precomputes `Tuning` outside timing. The integrated Rust measurement includes
coefficient updates and both live backends but not host-wrapper work.

To reproduce the CPU-native Linux configuration from the repository root,
first run `sh scripts/matrixverb-reference.sh` to fetch the pinned C++ source,
then:

```sh
(
  set -eu
  export CARGO_TARGET_DIR="$PWD/target/linux-native"
  export RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp -C target-cpu=native"
  mkdir -p "$CARGO_TARGET_DIR/reference"
  c++ -std=c++17 -O3 -march=native -fno-fast-math -ffp-contract=off \
    -fno-tree-vectorize -fno-tree-slp-vectorize \
    -include crates/matrixverb-dsp/tests/reference/MatrixVerb.h \
    target/matrixverb-reference/MatrixVerbProc.cpp \
    crates/matrixverb-dsp/tests/reference/runner.cpp \
    -o "$CARGO_TARGET_DIR/reference/matrixverb-reference"
  export MATRIXVERB_REFERENCE="$CARGO_TARGET_DIR/reference/matrixverb-reference"
  cargo test --locked --release --target x86_64-unknown-linux-gnu \
    --package oxidation-matrixverb-dsp --test reference -- --ignored --nocapture
  cargo build --locked --release --target x86_64-unknown-linux-gnu \
    --package oxidation-matrixverb-dsp --bin matrixverb-bench
  "$MATRIXVERB_REFERENCE" bench
  "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/matrixverb-bench"
  "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/matrixverb-bench" reverse
)
```

For the four-pass protocol in the root README, run the C++ command four
times and each Rust order twice, interleaving portable and native builds.
The native binaries require a CPU compatible with the build machine.

See the [main performance table](../../README.md#matrixverb-full-processor-performance)
for all measured results, per-tactic comparisons, integrated dual-backend
costs, and platform status. The root README is the sole canonical location
for performance results; this document retains methodology and reproduction
instructions only. SIMD/FMA is timed as one tactic, not as separately
isolated SIMD and FMA features.

## Profiling

The [profiling notes](../../README.md#profiling-driven-optimization) document
native samples, disassembly findings, and retained optimizations. Generic
vector constant construction had generated repeated memory-fill calls in
the sample loop. Explicit lane construction, native NEON rounding, and
sign masks eliminate those calls while preserving scalar-off code generation.

Build a symbolized release executable and isolate any mode for 12 seconds:

```sh
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --locked --release --package oxidation-matrixverb-dsp --bin matrixverb-bench
target/release/matrixverb-bench profile ah
```

Use `os`, `as`, `oh`, or `ah` for O/S, A/S, O/H, or A/H. The process prints
its PID; attach a native CPU profiler during the run. On macOS,
`sample PID 3 1 -file /tmp/matrixverb.sample.txt` captures stacks.
Do not use the profiling run as a published benchmark.
