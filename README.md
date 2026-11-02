# oxidation

**Air meets Rust.** oxidation is an independent Rust audio-plugin project
inspired by [Airwindows](https://github.com/airwindows/airwindows), created
by Chris Johnson.

Chris's algorithms, musical judgment, and generosity in publishing his work
make this project possible. The sound design is his contribution; our
experiment is how to express that DSP in idiomatic Rust and explore a
different set of implementation tradeoffs. This project celebrates that
foundation through careful porting, reproducible experiments, and shared
learning.
oxidation is not an official Airwindows release or an endorsed project.
Upstream MIT attribution is retained in
[LICENSE.airwindows](poc/matrix/LICENSE.airwindows).

## Test build status

**Ready for experimental Apple Silicon macOS and Linux x86_64 DAW testing.
Not yet a production release.** Both independent plugins build successfully
as CLAP and VST3 bundles, with separate host identities:

| Effect | Source package | CLAP output | VST3 output |
| --- | --- | --- | --- |
| Ultralight3 | [plugins/ultralight3](plugins/ultralight3) | `oxidation.clap` | `oxidation.vst3` |
| MatrixVerb | [plugins/matrixverb](plugins/matrixverb) | `oxidation-matrixverb.clap` | `oxidation-matrixverb.vst3` |

Build both from the repository root:

```sh
cargo test --locked --release --workspace
cargo xtask bundle oxidation --release --locked
cargo xtask bundle oxidation-matrixverb --release --locked
sh scripts/check-simd.sh
```

The four outputs are in `target/bundled/` by default; the
[Linux quick start](#linux-quick-start-build-and-share) uses
`target/linux/bundled/`. Share only the four names listed above; old `oxide`
or `oxide-windows` bundles left in a reused build directory are not part of
this test set. Preserve each bundle's complete directory structure when
copying or archiving it.

Local validation includes workspace tests, four-mode DSP and transition
tests, allocation checks, C++ reference comparisons, and packed SIMD/FMA
code-generation checks. Actual DAW scanning, playback, automation, and
session recall still need the [host test checklist](#first-daw-test).
Linux x86_64 portable and CPU-native tests, C++ reference comparisons,
CLAP/VST3 builds, and factory discovery checks passed on an i7-6700K.
Linux DAW use, Intel macOS, and Windows execution remain unverified.
macOS bundles are ad-hoc signed, not Developer ID signed or notarized.
Start at low monitoring volume in a disposable session.

Performance results and the effect of each optimization tactic are in the
[Ultralight3 table](#ultralight3-core-performance) and
[MatrixVerb table](#matrixverb-full-processor-performance) below.

## What we are demonstrating

Rust lets us describe exclusive access to preallocated audio state in a way
the compiler can verify. Disjoint mutable slices, statically dispatched
kernels, and explicit lifetimes make it practical to reorganize delay
processing without introducing shared mutable state or audio-thread
allocations. The goal is to turn those guarantees into measurable,
reproducible improvements while preserving and documenting the original
musical behavior.

Our initial isolated-matrix experiment measured comparable throughput in
Rust and C++. The delay-network experiment found an improvement
from borrowing contiguous regions of sixteen delay lines and processing
until the next wrap: fewer cursor updates, fewer checks in the sample loop,
and better visibility into non-aliasing. A copied-tile experiment was slower
and was discarded. Similar scheduling is possible in C++; Rust offers a
particularly useful, safely enforced way to express it.

Current Apple M4 Pro measurements show the original-order Rust
**Ultralight3 reverb core** at **1.52x C++ throughput in scalar mode** and
**2.69x with CPU SIMD/FMA**, with all feedback tone filters enabled.
The scalar core matched 94,080,000 external output-bit comparisons per
implementation; accelerated modes pass separate numerical error budgets.
See the [four-mode core table](#ultralight3-core-performance) and
[experiment methodology](poc/matrix/README.md).

The complete single-backend **MatrixVerb** processor reaches **1.32x C++
throughput with approximate math and SIMD/FMA**; its original-math modes
still trail C++. See the [four-mode processor table](#matrixverb-full-processor-performance)
and [DSP validation and methodology](crates/matrixverb-dsp/README.md).

These are **DSP measurements**, not whole-plugin speedups. The Linux
measurements compare portable and CPU-native builds on one i7-6700K; they
are not predictions for other CPUs. Compiler auto-vectorization is disabled;
only accelerated Rust modes use explicit SIMD/FMA. Both plugins run two live
backends for seamless math switching, adding work not represented by
single-backend comparisons.
The [profiling notes](#profiling-driven-optimization) explain the measured
optimizations and remaining limitations.

## Ultralight3 plugin

### Repository layout

Each plugin has a separate, visibly named package directory. The root
`Cargo.toml` is a virtual workspace; there is no implicit root plugin.

```text
plugins/
  ultralight3/   # Ultralight3 plugin (Cargo package: oxidation)
  matrixverb/   # MatrixVerb plugin (Cargo package: oxidation-matrixverb)
crates/
  matrixverb-dsp/
poc/matrix/     # Shared matrix/SIMD kernels and Ultralight3 delay core
```

See [Ultralight3 source](plugins/ultralight3) and
[MatrixVerb source](plugins/matrixverb). Existing build commands, bundle
names, and host IDs are unchanged by this layout.

oxidation is a full Rust port of Ultralight3's stereo `processReplacing`
signal path: the sixteen-delay feedback network, predelay and three
allpasses, damping and tone filters, front/back and floor/ceiling stereo
mix, dry/wet blend, FastDistance2 filters, internal-rate interpolation, and
32-bit output dither. The eight upstream controls and their default values
are retained. The implementation uses preallocated Rust state and does not
allocate in the audio callback.

Controls appear in the host's generic parameter UI; there is no custom editor:

| Control | Default | Behavior |
| --- | --- | --- |
| Damping | 2 | Steps 0–9; 0 bypasses the reverb network, 1–9 set decay damping. |
| UnSolid | 4 | Feedback high-pass selector, 0–4. |
| UnReflc | 4 | Feedback low-pass selector, 0–4. |
| Predlay | 0 | Upstream 0–998 index mapped through the same prime-delay scheme. |
| Dry/Wet | 0.5 | Dry signal and reverb blend. |
| Frnt/Bk | 0 | Selects between the front and back reverb taps. |
| Bot/Top | 0 | Selects between floor and ceiling reverb taps. |
| Distnce | 0 | FastDistance2 distance filtering. |
| Original-order math | On | Retains the validated scalar evaluation order. |
| Acceleration | Off | Enables CPU SIMD/FMA; may change rounding and is not guaranteed faster. |

The two Rust toggles produce four modes. Original-order math is the default;
approximate math balances a four-value reduction and is tested against an
error budget and long-tail stability checks. Acceleration is independently
off by default. When on, it uses four-wide `f32` SIMD/FMA for the delay
network, feedback filters, predelay/allpasses, distance filters and wet/dry
mixing. On Apple Silicon this uses NEON; on x86_64 it uses SSE2 with
runtime-checked hardware FMA. CPUs without FMA use an explicitly reported
scalar fused fallback for fused operations, while retaining SIMD for other
vector arithmetic. Switching Original-order
math crossfades the continuously running node histories over 50 ms.
Changing Acceleration selects SIMD/FMA immediately and does not
reset the delay state.
The workspace disables compiler auto-vectorization so that acceleration
off remains scalar/unfused. Acceleration on uses explicit SIMD, independent
of those compiler settings. SIMD and FMA are CPU features, not GPU/Metal
offload; acceleration is an implementation choice, not a speed guarantee.

The internal reverb rate is 48 kHz. Its causal linear sample-rate adapter
supports host rates from 8 to 384 kHz; this is not a band-limited mastering
resampler and may alias, especially below 48 kHz. The adapter and
deterministic dither seeds are Rust implementation choices, so whole-plugin
output is not claimed to be bit-identical to the upstream VST. The
original-order sixteen-delay node is covered by the external C++ bitwise
oracle. NIH-plug enables flush-to-zero on supported x86_64/AArch64 targets.
The plugin requests continuous host processing so that reverb tails are not
truncated by silence-based suspension.

### Ultralight3 core performance

This root README is the canonical source for all performance results and
per-tactic comparisons. DSP sub-READMEs document implementation, validation,
and reproduction commands, and link here rather than duplicating results.

C++ and Rust measurements, in nanoseconds per native-rate sample:
256-frame blocks, damping step 5, both tone filters enabled. Apple values
average two 11-round medians, with Rust mode order reversed on the second
run. Linux values are the median of four 11-round medians, alternating Rust
mode order and balancing build order. This is a core-only comparison, not
whole-plugin CPU usage. Lower is faster.

| C++ | O/S | A/S | O/H | A/H |
| ---: | ---: | ---: | ---: | ---: |
| Apple M4 Pro: 14.745 | 9.716 | 9.249 | 5.489 | 5.529 |
| Linux i7-6700K, portable: 27.068 | 22.031 | 21.206 | 58.450 | 58.415 |
| Linux i7-6700K, CPU-native: 26.056 | 22.048 | 22.362 | 15.880 | 16.256 |

**Legend:** C++ = original reference; O = original math; A = approximate
math; S = scalar, unfused; H = CPU SIMD/FMA acceleration.

**Measured effect of each tactic on this M4 Pro** (processing-time reductions,
computed from the table above):

- Approximate math alone, O/S -> A/S: 9.716 -> 9.249 ns/sample,
  **4.8% less time**.
- SIMD/FMA with original math, O/S -> O/H: 9.716 -> 5.489,
  **43.5% less time (1.77x throughput)**.
- SIMD/FMA with approximate math, A/S -> A/H: 9.249 -> 5.529,
  **40.2% less time (1.67x throughput)**.
- Approximate math with SIMD/FMA already enabled, O/H -> A/H:
  5.489 -> 5.529, **0.7% more time**; no measured additional speedup.
- Both tactics versus original/scalar, O/S -> A/H: 9.716 -> 5.529,
  **43.1% less time (1.76x throughput)**.

SIMD/FMA is the dominant measured improvement here; approximate math adds
no speed benefit to the accelerated path. Small differences are descriptive
of these runs, not statistically established effects. Overlapping min/max
timing ranges do not negate the large measured SIMD/FMA improvement.

On this i7, O/H changes from 58.450 to 15.880 ns/sample between the portable
and CPU-native builds, or **3.68x** the throughput. Both builds reported
SSE2 SIMD with hardware FMA. Native targeting changes code generation for
the whole build, so this comparison does not isolate AVX2, FMA, or a single
optimization. The portable Rust benchmark command is:

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --locked --release --manifest-path poc/matrix/Cargo.toml --bin node-bench
```

The C++ measurements and experimental methodology are documented in the
[DSP experiment](poc/matrix/README.md), including CPU-native reproduction.
Treat small differences as machine- and run-dependent; no whole-plugin
speedup is implied.
After profiling and removing hot-loop overhead, O/H is about **1.77x**
the scalar O/S throughput and **2.69x** the C++ core throughput on this M4 Pro.
A reproducible fresh C++ comparison is available through
`sh scripts/ultralight-reference.sh`; each Rust benchmark mode starts from
reset state before identical warmup.

## MatrixVerb plugin

**oxidation MatrixVerb** is a separate stereo CLAP/VST3 plugin; the existing
Ultralight3 plugin and its host identities are unchanged. It ports the pinned
Airwindows MatrixVerb processor, not just its matrix kernel: stereo predelay,
four parallel input allpasses, eight modulated delay lines per channel,
Householder feedback, sequential A/E crossmodulation, three stereo biquads,
sine/arcsine encoding, upstream dry/wet behavior, denormal input noise and
32-bit output dither. Internal math and delay storage use `f64`.

All seven original normalized controls retain their upstream defaults:

| Control | Default |
| --- | ---: |
| Filter | 1 |
| Damping | 0 |
| Speed | 0 |
| Vibrato | 0 |
| RmSize | 0.5 |
| Flavor | 0.5 |
| Dry/Wet | 1 |

Original-order math defaults on; Acceleration defaults off. Original math
retains the upstream `f64` sine/arcsine and matrix evaluation order.
Approximate math replaces sine/arcsine with range-reduced polynomials and
balances matrix/output reductions. Acceleration independently selects
two-wide `f64` SIMD/FMA across left/right channels: biquads, allpasses,
delay interpolation, feedback matrices and mixing. The approximate backend
also evaluates its polynomials in SIMD. Original math retains the upstream
scalar libm sine/arcsine rather than silently substituting approximations.
Both backends use the CPU capability handling described above. Generic
specialization selects the scalar or SIMD kernel outside the sample loop.

Both backends run continuously, with a reversible 50 ms crossfade for the
math toggle. Acceleration switches immediately without clearing histories.
Processing and reset allocate no memory. Host sample rates from 22.05 to
384 kHz are supported; unlike Ultralight3, MatrixVerb runs its delay network
directly at the host rate, matching the reference's sample-rate behavior.
NIH-plug supplies the host floating-point denormal handling. The standalone
DSP processor inherits its caller's floating-point environment.

Startup modulation phases and noise seeds are deterministic instead of the
upstream constructor's platform-dependent `rand()` initialization. The
seeded full-path C++ comparison covers float and double processing, six
sample rates, ten control/silence/automation scenarios, and 7,864,320 output
samples per mode, covering all four modes. On this M4 Pro, original math with acceleration off produced **zero
observed error** in both precisions. This is finite-corpus evidence under
matching initial state, not a guarantee of matching an arbitrary upstream
instance or another platform bit-for-bit. NIH-plug's host wrapper processes
32-bit buffers; the double path is available in the DSP library/test harness.

### MatrixVerb full-processor performance

Measurements in median **ns/stereo frame**, 44.1 kHz, default controls,
256-frame blocks, 11 measured rounds after warmup. The Apple and Linux
aggregation and order follow the Ultralight3 table above. Measurements
include the entire single-backend signal path and dither, not just the
Householder matrix.

| C++ | O/S | A/S | O/H | A/H |
| ---: | ---: | ---: | ---: | ---: |
| Apple M4 Pro: 78.980 | 99.693 | 97.255 | 97.360 | 59.900 |
| Linux i7-6700K, portable: 348.858 | 488.378 | 549.000 | 580.518 | 824.966 |
| Linux i7-6700K, CPU-native: 324.022 | 399.396 | 393.682 | 362.160 | 281.246 |

**Legend:** C++ = original reference; O = original math; A = approximate
math; S = scalar, unfused; H = CPU SIMD/FMA acceleration.

On the i7, the single-backend A/H result changes from 824.966 to 281.246
ns/stereo frame between portable and CPU-native builds, or **2.93x** the
throughput. This is a comparison of complete build configurations, not an
isolated measurement of individual CPU instructions.

**Measured effect of each tactic on this M4 Pro** (single-backend
processing-time reductions, computed from the table above):

- Approximate math alone, O/S -> A/S: 99.693 -> 97.255 ns/stereo frame,
  **2.4% less time**.
- SIMD/FMA with original math, O/S -> O/H: 99.693 -> 97.360,
  **2.3% less time**.
- SIMD/FMA with approximate math, A/S -> A/H: 97.255 -> 59.900,
  **38.4% less time (1.62x throughput)**.
- Approximate math with SIMD/FMA already enabled, O/H -> A/H:
  97.360 -> 59.900, **38.5% less time (1.63x throughput)**.
- Both tactics versus original/scalar, O/S -> A/H: 99.693 -> 59.900,
  **39.9% less time (1.66x throughput)**.

The tactics interact: neither alone shows a large improvement here, but
their combination does. The roughly 2% differences are small observed
changes, not statistically established wins; they must not be used to
dismiss the roughly 40% reduction achieved by combining the tactics.
These comparisons measure the combined SIMD/FMA tactic, not SIMD and FMA
individually; isolating those would require additional benchmark variants.

Approximate SIMD/FMA (A/H) delivers about **1.62x** the A/S throughput and
**1.32x** the C++ reference throughput on the M4 Pro. Original math,
including O/H, still trails C++ there; SIMD does not replace its libm calls.
The actual plugin DSP runs **both** backends for seamless switching.
Its separately measured M4 Pro costs were 208.601 / 207.549 / 161.048 /
160.603 ns/stereo frame for O/S / A/S / O/H / A/H, excluding host-wrapper
overhead. The math toggle selects the output rather than stopping the
unselected backend, so similar integrated O/A timings are expected.
Do not equate single-backend figures with plugin CPU usage.

The Linux integrated dual-backend DSP costs, in the same O/S / A/S / O/H /
A/H order, were 1053.320 / 1046.870 / 1426.086 / 1400.298 ns/stereo frame
for the portable build and 794.782 / 805.354 / 638.499 / 643.148 for the
CPU-native build. These also exclude host-wrapper overhead.

The Apple rows use Rust 1.94.0 and Apple Clang 21.0.0 with portable target
defaults. Linux measurements were taken on 2026-11-02 on Bazzite 44 with an
Intel Core i7-6700K, Rust 1.95.0 and GCC 16.2.1. Linux builds use portable
defaults or `-C target-cpu=native` for Rust and `-march=native` for C++.
All configurations disable compiler auto-vectorization; the C++ builds also
use `-O3`, `-fno-fast-math`, `-ffp-contract=off`, and no LTO, while Rust uses
release-profile fat LTO.
Native builds may require CPU features unavailable on other machines.

Apple runs were not CPU-pinned; Linux runs were pinned to logical CPU 3,
whose governor reported `performance` when checked. Neither machine was
isolated from other workloads. Among the Linux results, the four portable
C++ MatrixVerb run medians had an 8.3% spread relative to their median, so
small differences should not be treated as conclusive.
The MatrixVerb C++ benchmark recomputes block coefficients; its Rust
single-backend benchmark precomputes them, while integrated Rust timing
includes coefficient updates.

The DSP library and reproducible reference/benchmark instructions are in
[MatrixVerb DSP](crates/matrixverb-dsp/README.md). To download the pinned
MIT-licensed upstream source, compile its unmodified processor with a seeded
state shim, compare full outputs and benchmark all four modes:

```sh
sh scripts/matrixverb-reference.sh
```

This optional comparison requires `curl` and a C++17 compiler on macOS/Linux.
Normal plugin builds and Rust tests need neither C++ nor reference downloads.
The C++ comparison is explicitly ignored in plain `cargo test` and run by
the script and macOS/Linux CI instead.

### Profiling-driven optimization

Native macOS `sample` captures and optimized disassembly identified repeated
`memset_pattern16` calls when `wide` constructed SIMD constants with
auto-vectorization disabled. In the initial MatrixVerb A/H capture, roughly
60% of samples were in those calls or their stubs. Explicit lane construction
removed that overhead without enabling auto-vectorization in scalar mode.
NEON nearest-even rounding replaces the vector library's multi-operation
rounding; sign masks avoid generic constant construction. Follow-up
disassembly confirms no memory-fill calls in the single-processor kernels.

Ultralight3 also keeps feedback/filter vectors in registers across the
borrowed-window loop, stores them at block exit, and constructs tone masks
once per block. Inactive filter histories and live acceleration changes are
preserved and regression-tested. MatrixVerb's approximate sine folds its
range to [-pi/2, pi/2] and uses a degree-15 Estrin polynomial to shorten
arithmetic dependency chains; original libm math is unchanged.

Compared with the first SIMD implementation, accelerated Ultralight3 fell
from about 16.1 to 5.5 ns/sample and approximate accelerated MatrixVerb from
163.8 to 59.9 ns/stereo frame. Those before/after figures describe the
implementation improvement, not controlled cross-machine measurements.
The tables above replace the initial results with fresh forward/reverse
runs. All four modes still pass the C++ error budgets; original/scalar
MatrixVerb has zero observed error, and the Ultralight3 scalar node retains
94,080,000 bit-identical comparisons per implementation.

For isolated 12-second profiling workloads (modes `os`, `as`, `oh`, `ah`):

```sh
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --locked --release --package oxidation-matrixverb-dsp --bin matrixverb-bench
target/release/matrixverb-bench profile ah
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --locked --release --manifest-path poc/matrix/Cargo.toml --bin node-bench
poc/matrix/target/release/node-bench profile oh
```

Each workload prints its PID. On macOS, attach from another terminal with
`sample PID 3 1 -file /tmp/oxidation.sample.txt`. Full Xcode's Time Profiler
was unavailable on this machine, so these captures used the native sampler.
Run benchmarks without the profiler or competing workloads.
Original MatrixVerb still has scalar transcendental calls, vector/scalar
transitions, and delay bookkeeping; this work does not establish a speedup
for its original-math backend over C++. Native Intel/Linux profiling and
actual-host callback measurements remain pending.

## Roadmap

- Validate the plugin in real hosts, measure whole-plugin CPU and callback
  timing, improve sample-rate conversion, and publish cross-platform results.
- Extend native profiling to Intel/Linux and measure actual-host callback
  latency and dual-backend overhead before claiming whole-plugin speedups.
- Extend to other Airwindows algorithms.

## Build targets

- macOS: Apple Silicon (`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`).
- Windows: x64 MSVC (`x86_64-pc-windows-msvc`).
- Linux: x64 GNU (`x86_64-unknown-linux-gnu`).

### Linux quick start: build and share

Open or copy the complete checkout on an **x86_64 GNU/Linux** machine.
Include both Cargo lockfiles, `xtask/`, `.cargo/`, `plugins/`, `crates/`,
and `poc/matrix/`; the plugins use the Rust libraries in those directories.
You do not need any
existing `target/` directories or the external C++ reference harness.
An internet connection is needed for the initial dependency downloads.

On Debian/Ubuntu, install the native build prerequisites:

```sh
sudo apt-get update
sudo apt-get install build-essential git curl ca-certificates
```

On Fedora, use `sudo dnf install gcc gcc-c++ binutils git curl ca-certificates`.
Install the current stable Rust toolchain using
[rustup's official instructions](https://rustup.rs/), then open a fresh
terminal so `cargo` and `rustc` are on `PATH`.

**From the checkout root, this is the build-and-test command:**

```sh
sh scripts/build-linux.sh
```

The script runs the plugin and DSP tests in release mode, then bundles both
plugins in both formats. It disables compiler auto-vectorization for this scalar
experiment, uses locked dependencies and portable target defaults rather
than `target-cpu=native`, and stops on a failed test or build. It requires a
native x86_64 GNU/Linux Rust host; ARM Linux and musl are not validated targets.
It does not install plugins or change system packages.

Outputs are separated from other platforms:

```text
target/linux/bundled/oxidation.clap
target/linux/bundled/oxidation.vst3/
target/linux/bundled/oxidation-matrixverb.clap
target/linux/bundled/oxidation-matrixverb.vst3/
```

For a separate CPU-native build, run:

```sh
(
  set -eu
  export CARGO_TARGET_DIR="$PWD/target/linux-native"
  export RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp -C target-cpu=native"
  cargo xtask bundle oxidation --release --locked --target x86_64-unknown-linux-gnu
  cargo xtask bundle oxidation-matrixverb --release --locked --target x86_64-unknown-linux-gnu
)
```

Its four outputs are in `target/linux-native/bundled/`. This build can use
any CPU feature available on the build machine and may not run on older or
otherwise incompatible CPUs. Use the portable `target/linux/bundled/` build
when sharing with machines of unknown capability. Both builds contain all
four processing modes; the portable build also supports runtime-selected
SIMD/FMA acceleration.

To install for the current user, close your DAW first, then:

```sh
mkdir -p "$HOME/.clap" "$HOME/.vst3"
cp target/linux/bundled/oxidation.clap "$HOME/.clap/"
cp -R target/linux/bundled/oxidation.vst3 "$HOME/.vst3/"
cp target/linux/bundled/oxidation-matrixverb.clap "$HOME/.clap/"
cp -R target/linux/bundled/oxidation-matrixverb.vst3 "$HOME/.vst3/"
```

These commands replace an existing same-named plugin. Preserve any previous
test build separately if you need to compare versions. To install the
CPU-native build instead, substitute `target/linux-native/bundled/` for
`target/linux/bundled/` in those commands. The builds share plugin IDs and
filenames, so keep only one build of each plugin in the DAW's scan paths at
a time. Alternatively, configure the DAW to scan one bundle directory
directly.
Restart/rescan the DAW and look for **oxidation** and **oxidation MatrixVerb**.
Test either CLAP or VST3 first; each format contains the same two effects.

For a friend who does not want to compile, zip the four Linux outputs
together **after the script passes on Linux**, retaining the complete VST3
directory structure. A locally built GNU/Linux binary is not guaranteed to
load on distributions with an older glibc; building on the friend's own
machine avoids that particular mismatch.

#### First DAW test

This is an experimental plugin. Start with low monitoring volume in a
disposable session, not a production project.

Each bundle includes all four modes. Use the DAW's generic parameter editor
to set the two controls:

| Mode | Original-order math | Acceleration |
| --- | --- | --- |
| O/S | On | Off |
| A/S | Off | Off |
| O/H | On | On |
| A/H | Off | On |

1. Confirm scanning/loading works and stereo audio passes for both plugins
   in each format your DAW supports.
2. Test Damping, UnSolid, UnReflc, Predlay, Dry/Wet, Frnt/Bk, Bot/Top and
   Distnce on Ultralight3. On MatrixVerb, test Filter, Damping, Speed,
   Vibrato, RmSize, Flavor and Dry/Wet.
3. Try all four modes. Switch Original-order math during a sustained tail,
   including switching back before its 50 ms transition finishes; listen for
   discontinuities.
4. Toggle Acceleration both ways during a sustained tail and confirm it
   changes the processing mode without clearing the reverb state.
5. Save/reopen the session and confirm parameter and mode recall.
6. Try 44.1/48/96 kHz and several host buffer sizes; watch CPU and output.
   When comparing builds, use the same audio, settings, instance count, sample
   rate, and buffer size. Host CPU usage is not the core benchmark below.
7. Report the distro, architecture, glibc version (`ldd --version`), DAW and
   version, plugin format, portable or CPU-native build, sample rate, buffer
   size, and reproduction steps. Include the terminal error output if
   building or scanning fails.

Automated tests do not substitute for this host test. Linux CLAP and VST3
bundles were built and inspected, but DAW compatibility remains unverified.

### Other native builds

Install stable Rust and the native platform linker/toolchain (Xcode Command
Line Tools on macOS, Visual Studio C++ Build Tools on Windows, or a native
build toolchain on Linux). The DSP is Rust-only; C++ is not a build or
runtime dependency. NIH-plug supplies the plugin-format wrappers.

```sh
cargo test --locked --workspace
cargo xtask bundle oxidation --release --locked
cargo xtask bundle oxidation-matrixverb --release --locked
```

Installable CLAP and VST3 bundles are written to `target/bundled/`.
On macOS/Linux, `sh scripts/check-simd.sh` checks those actual CLAP binaries
for packed FMA instructions in both plugins; CI runs the same check.
Numerical and routing tests separately verify all four configurations.
No standalone application or Audio Unit build is configured.
Release signing/notarization is not configured; macOS bundles receive local
ad-hoc signing rather than Developer ID signing.
The oxidation CLAP ID is `audio.oxidation.reverb` and its VST3 class ID is
`Oxidation0000001`. This product rename intentionally creates new host
identities; projects using the prior oxide-windows build may need their
plugin instance replaced.
MatrixVerb has its own CLAP ID `audio.oxidation.matrixverb`, VST3 class ID
`OxidationMatrix1`, and `oxidation-matrixverb` bundle filenames.

For a universal macOS bundle:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask bundle-universal oxidation --release --locked
cargo xtask bundle-universal oxidation-matrixverb --release --locked
```

[Build CI](.github/workflows/build.yml) tests the plugin and scalar DSP
and bundles both formats on each platform. Other platform builds remain
unverified until those jobs run successfully.
The workspace disables auto-vectorization through `.cargo/config.toml`;
explicit SIMD remains available in accelerated modes. If overriding
`RUSTFLAGS`, retain `-C no-vectorize-loops -C no-vectorize-slp` to preserve
the scalar/accelerated distinction. The workspace folder's name need not
match the app.
