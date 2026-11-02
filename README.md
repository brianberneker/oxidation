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

On one Apple M4 Pro, the original-order Rust **Ultralight3 reverb core**
measured about **2.5–2.7x the throughput** of an extracted, unchanged C++
core with feedback tone filters disabled, and **1.6–1.7x** with all filters
enabled. Both references used scalar arithmetic, with vectorization
disabled and no FMA contraction. The Rust core matched 94,080,000 external
output-bit comparisons across settings and block sizes.

These are **core-only measurements**, not a whole-plugin speedup, an x86
result, or a comparison of FMA implementations. The plugin
below also runs two backends for seamless switching and includes additional
sample-rate and mixing work. See the [experiment and methodology](poc/matrix/README.md)
before quoting the numbers.

## The plugin

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
| Acceleration | Off | Uses fused multiply-add arithmetic when enabled; may change rounding. |

The two Rust toggles produce four modes. Original-order math is the default;
approximate math balances a four-value reduction and is tested against an
error budget and long-tail stability checks. Acceleration is independently
off by default and uses `f32::mul_add()` in the delay network, filters and
mixing path. Rust selects hardware FMA where the target supports it; no SIMD
intrinsics or optional CPU requirement are used. FMA changes rounding and
is not guaranteed to be faster on every target. Switching Original-order
math crossfades the continuously running node histories over 50 ms.
Changing Acceleration selects fused arithmetic immediately and does not
reset the delay state.

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

The table reports nanoseconds per native-rate sample for a 256-frame block
with damping step 5 and both tone filters enabled. Each Rust value is the
average of the medians from two `node-bench` runs, one in forward toggle
order and one in reverse order, on the Apple M4 Pro. The C++ column is the
previously measured extracted upstream core. The Rust timing ranges overlap,
so these measurements do not establish a performance benefit from either
toggle. This is a core-only comparison, not whole-plugin CPU usage. Lower is
faster.

| Original C++ reference | Original math / acceleration off | Approximate math / acceleration off | Original math / acceleration on | Approximate math / acceleration on |
| ---: | ---: | ---: | ---: | ---: |
| Apple M4 Pro: 14.591 | 9.107 | 9.172 | 9.142 | 9.215 |
| Linux x86_64: — | — | — | — | — |

Linux values are pending a native Linux run. The Rust benchmark command is:

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release --manifest-path poc/matrix/Cargo.toml --bin node-bench
```

The C++ measurements and experimental methodology are documented in the
[DSP experiment](poc/matrix/README.md). Treat small differences as
machine- and run-dependent; no x86 result or whole-plugin speedup is implied.

## Roadmap

- Validate the plugin in real hosts, measure whole-plugin CPU and callback
  timing, improve sample-rate conversion, and publish cross-platform results.
- Extend the shared test harness and modular backend approach to MatrixVerb
  and other Airwindows algorithms.

## Build targets

- macOS: Apple Silicon (`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`).
- Windows: x64 MSVC (`x86_64-pc-windows-msvc`).
- Linux: x64 GNU (`x86_64-unknown-linux-gnu`).

### Linux quick start: build and share

Open or copy the complete checkout on an **x86_64 GNU/Linux** machine.
Include both Cargo lockfiles, `xtask/`, `.cargo/`, and `poc/matrix/`;
the plugin uses the Rust library in that directory. You do not need any
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
plugin formats. It disables compiler auto-vectorization for this scalar
experiment, uses locked dependencies and portable target defaults rather
than `target-cpu=native`, and stops on a failed test or build. It requires a
native x86_64 GNU/Linux Rust host; ARM Linux and musl are not validated targets.
It does not install plugins or change system packages.

Outputs are separated from other platforms:

```text
target/linux/bundled/oxidation.clap
target/linux/bundled/oxidation.vst3/
```

To install for the current user, close your DAW first, then:

```sh
mkdir -p "$HOME/.clap" "$HOME/.vst3"
cp target/linux/bundled/oxidation.clap "$HOME/.clap/"
cp -R target/linux/bundled/oxidation.vst3 "$HOME/.vst3/"
```

These commands replace an existing same-named plugin. Preserve any previous
test build separately if you need to compare versions. Alternatively,
configure the DAW to scan `target/linux/bundled/` directly.
Restart/rescan the DAW and look for **oxidation**. Test either CLAP or
VST3 first; both contain the same effect.

For a friend who does not want to compile, zip the two Linux outputs
together **after the script passes on Linux**, retaining the complete VST3
directory structure. A locally built GNU/Linux binary is not guaranteed to
load on distributions with an older glibc; building on the friend's own
machine avoids that particular mismatch.

#### First DAW test

This is an experimental plugin. Start with low monitoring volume in a
disposable session, not a production project.

1. Confirm scanning/loading works and stereo audio passes.
2. Test Damping, UnSolid, UnReflc, Predlay, Dry/Wet, Frnt/Bk, Bot/Top and
   Distnce.
3. Switch Original-order math during a sustained tail, including switching
   back before its 50 ms transition finishes; listen for discontinuities.
4. Toggle Acceleration both ways during a sustained tail and confirm it
   changes the processing mode without clearing the reverb state.
5. Save/reopen the session and confirm parameter and mode recall.
6. Try 44.1/48/96 kHz and several host buffer sizes; watch CPU and output.
7. Report the distro, architecture, glibc version (`ldd --version`), DAW and
   version, plugin format, sample rate, buffer size, and reproduction steps.
   Include the terminal error output if building or scanning fails.

Automated tests do not substitute for this host test. Linux build and
DAW compatibility remain unverified until run on Linux.

### Other native builds

Install stable Rust and the native platform linker/toolchain (Xcode Command
Line Tools on macOS, Visual Studio C++ Build Tools on Windows, or a native
build toolchain on Linux). The DSP is Rust-only; C++ is not a build or
runtime dependency. NIH-plug supplies the plugin-format wrappers.

```sh
cargo test --locked --package oxidation
cargo xtask bundle oxidation --release --locked
```

Installable CLAP and VST3 bundles are written to `target/bundled/`.
No standalone application or Audio Unit build is configured.
Release signing/notarization is not configured; macOS bundles receive local
ad-hoc signing rather than Developer ID signing.
The oxidation CLAP ID is `audio.oxidation.reverb` and its VST3 class ID is
`Oxidation0000001`. This product rename intentionally creates new host
identities; projects using the prior oxide-windows build may need their
plugin instance replaced.

For a universal macOS bundle:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask bundle-universal oxidation --release --locked
```

[Build CI](.github/workflows/build.yml) tests the plugin and scalar DSP
and bundles both formats on each platform. Other platform builds remain
unverified until those jobs run successfully.
For explicitly scalar builds/experiments, set
`RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp"`; the default compiler
may otherwise auto-vectorize eligible code, without requiring optional
native CPU features. The workspace folder's name need not match the app.
