# oxidation: scalar matrix and delay-network POC

Independent, dependency-free Rust implementations of one four-input matrix
transform from each of MatrixVerb and Ultralight3. This is not a full plugin
port, a delay-network model, or a demonstration of whole-plugin speedup.
The later Ultralight3 delay-network experiment below validates the
sixteen-delay core. The root oxidation plugin now consumes this Rust library
as part of a complete Ultralight3 signal-path port and adds selectable
approximation and fused-arithmetic modes.

## Source and numerical contract

Reference revision: `7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a`.

- [MatrixVerb reference](https://github.com/airwindows/airwindows/blob/7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a/plugins/WinVST/MatrixVerb/MatrixVerbProc.cpp):
  double-precision four-input feedback transform, including regeneration.
- [Ultralight3 reference](https://github.com/airwindows/airwindows/blob/7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a/plugins/WinVST/Ultralight3/Ultralight3Proc.cpp):
  single-precision four-input transform before a delay stage.

Preserve each expression's original floating-point association and precision.
Do not introduce fast math, explicit FMA, a balanced reduction, precision
reduction, or extra denormal offsets. There are no recursive state nodes in
these isolated kernels. Full-plugin denormal handling is outside this POC.

Rust separates immutable input and exclusive output slices, checks their
lengths before iteration, and allocates nothing in the kernels. MatrixVerb
shares identical pair sums without changing association. Ultralight3 already
shares a reduction in the upstream implementation; no algorithmic improvement
is claimed for it. LLVM may already perform the MatrixVerb common-subexpression
elimination in C++, too.

The block entry points deliberately remain out of line for assembly inspection
and to prevent benchmark elimination; single-transform kernels are always
inlined. Benchmark buffers and measurement storage are allocated outside the
processing path.

## Build, test, and measure

From this directory, with a Rust toolchain installed:

```sh
cargo fmt --check
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo test --release
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo rustc --release --lib -- --emit=asm
```

Both LLVM vectorizers are disabled for the scalar-only experiment. Inspect
the emitted kernel assembly as well; on AArch64 scalar FP instructions use
the same register file as vector instructions, so register names alone do
not identify SIMD. No optional CPU features or native-target flags are enabled.

The benchmark reports elapsed nanoseconds per four-input transform, with a
warmup and 21 measured rounds over reused 4096-frame buffers. This is a
cache-resident, independent-transform throughput test, not a latency test
or a realistic delay-line workload. Compare on the same machine, without
parallel benchmark processes. Treat differences within run-to-run noise as
inconclusive. A borrow-checker guarantee is not a speedup guarantee.

## External C++ oracle

The Rust build neither includes nor links C++ and requires no C++ toolchain.
An external reference runner may compile the original matrix expressions with
optimizations enabled, vectorization disabled, no fast math and FP contraction
off. Such a runner must not alter arithmetic or precision. Keep identical
inputs, block sizes, repetition counts, and numerical modes for comparisons.
Both references need distinct input/output buffers; report any aliasing
assumptions or compiler differences.

To check an externally generated oracle file:

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release -- verify REFERENCE.bin
```

Binary format, all little-endian:

- Eight-byte magic `AWMATR01`, then a `u64` record count.
- Each record: four `f64` MatrixVerb inputs, one `f64` regeneration,
  four expected `f64` outputs, four `f32` Ultralight3 inputs, four expected
  `f32` outputs.

Every output is compared by raw bits, including signed zero. An empty,
truncated, mismatching, or trailing-data file is rejected. Inputs with NaN
payloads do not have a portable bit-identity guarantee across compilers.
Passing a finite corpus is evidence, not proof for all IEEE-754 values.

The root oxidation plugin now includes those full signal-path stages.
These POC measurements still cover only the matrix kernels and node, not
whole-plugin bit identity, whole-plugin speedup, or performance on another
CPU; those claims require separate plugin-level comparison and measurement.

## Initial local results

Measured on Apple M4 Pro, arm64 macOS, with Rust 1.94.0 and Apple Clang 21.0.0.
The external C++ runner compiled extracted original expressions, not the
complete original plugins, using `-O3 -fno-fast-math -ffp-contract=off
-fno-vectorize -fno-slp-vectorize`. Its kernels and timing driver were separate
translation units without LTO. Rust used the release profile and flags above.
Neither build used native CPU tuning or explicit SIMD. C++ pointers were
unqualified; Rust slices carry their normal non-aliasing guarantees.

Two sequential passes, with benchmark order reversed:

| Kernel | C++ median ns/transform | Rust median ns/transform |
| --- | --- | --- |
| MatrixVerb, pass 1 | 0.922 | 0.914 |
| MatrixVerb, pass 2 | 0.924 | 0.917 |
| Ultralight3, pass 1 | 0.674 | 0.676 |
| Ultralight3, pass 2 | 0.677 | 0.675 |

These are approximately equal throughput, not a meaningful demonstrated
speedup. Observed round-to-round variation was larger than the differences.
Assembly inspection found scalar arithmetic, no FMA or packed arithmetic,
and no per-sample bounds checks in the Rust loops. Both languages generated
the same number of FP arithmetic operations: MatrixVerb used seven adds,
four subtracts and four multiplies; Ultralight3 used seven adds and four
subtracts (multiplication by two became scalar addition). Scheduling,
loads/stores and entry checks differed.

The external C++ oracle generated 200,000 records spanning signed zeros,
subnormals, finite exponent ranges and six regeneration values. All 1,600,000
output bit patterns matched. The test suite also checks cancellation,
block-size handling, malformed oracle files and a driven matrix recurrence.
The recurrence is not a full reverb tail or delay-network stability test.

This result does not support a claim that switching to Rust alone vastly
outperforms these C++ matrix kernels. The figures above use the scalar
non-FMA path; the separate four-mode node benchmark below covers fused
arithmetic. Performance on x86 still requires a native measurement.

## Ultralight3 delay-network experiment

The POC now also ports all sixteen matrix delay lines (A through P), all
four matrix stages, the one-sample feedback register, damping, all five
UnSolid selectors and all five UnReflect selectors. It accepts mono input
at the node's native update rate and produces eight floor/ceiling outputs.
It preserves the pinned revision's delay lengths and initial state.

Excluded from this reference-core comparison: Q/R/S/T predelay/allpasses,
stereo input folding, host-rate interpolation, distance filtering, output
mixing, dither and plugin/UI integration. Input is the upstream `oQ` value
after predelay/bypass.
Do not treat these results as whole-plugin throughput or fidelity.

### The optimization

`OriginalNode` is a safe, samplewise Rust baseline following the original
write/advance/read ring behavior. `BorrowedNode` uses compact FIFO rings:
the original length-plus-one ring implements exactly a length-sample delay,
so the unused write slot can be removed without changing observable samples.
There is no change to matrix arithmetic, precision or feedback ordering.

For each processing span, choose the minimum contiguous space remaining in
all sixteen rings and the input block. Borrow those sixteen disjoint mutable
slices and zip them with distinct input/output slices. Read each old delayed
value, then replace it with its new input. The windows are exact length, so
the inner loop has no per-sample ring wrap or delay-array index check.
Each cursor advances once per span, not once per sample. No scratch-buffer
copies, heap allocation, unsafe DSP code, dynamic dispatch, SIMD intrinsics,
FMA, fast math or transcendental approximations are involved.

The borrow checker establishes disjointness and safe lifetimes; it is not
itself a runtime optimization pass. Removing cursor/index overhead and
exposing useful alias information is the concrete optimization here.
An equivalent redesign is possible in C++; these results are not evidence
that this speedup is exclusive to Rust.

Feedback tone filters intentionally preserve the original expression order
and selector behavior, including retaining the state of inactive filters.
The node inherits its caller's floating-point environment. No extra denormal
offset is injected because that would violate bit identity. Production host
integration must guarantee compatible FTZ/DAZ (or equivalent) for both the
reference and Rust, and separately validate silent-tail timing. This
standalone core does not configure the host's floating-point environment.

### Running the experiment

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo test --release
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo clippy --release --all-targets -- -D warnings
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release --bin node-bench
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release --bin node-bench -- reverse
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release --bin node-bench -- verify REFERENCE.bin
```

`reverse` measures the optimized Rust version before the baseline, rather
than after it. Each node is warmed up and then measured for 11 rounds of
524,288 samples per round, with 16/64/256/1024-sample blocks and filters
either all inactive or all active. State continues between blocks and rounds;
it is never reset inside a timed section. Buffers and state are preallocated.
The input is periodic: `((sample * 17 % 97) - 48) * 0.0001` in f32, repeating
at each host block. This is one warmed node, not a many-instance cache test,
a worst-case callback-latency measurement or an x86 portability result.

### External reference validation

The C++ reference was built outside the Rust workspace with Apple Clang 21
and the same scalar flags listed above. Its A-through-P delay/feedback
processing statements were extracted unchanged from the pinned
`processReplacing` method, starting at `aA[cA] = fA*damping + oQ` and ending
after the UnReflect switch. The harness supplies `oQ` directly, zeroes state,
starts cursors at one, and emits hA through hH before plugin output mixing.
It retains the original member-array declarations and uses a no-inline
processing boundary. The harness is validation infrastructure only: the
Rust build has no C++ source, FFI, compiler requirement or dependency.

Reference source SHA-256:
`095f416d473f9fe9f3b256265d926f8566591c0350a9068907b470d9507d8827`.

The oracle corpus covers all 25 tone-selector combinations at damping steps
1, 5 and 9. There are 20,000 frames per case except the first case, which
contains a 480,000-frame impulse tail (10 seconds at 48 kHz). Equal tone
selectors use an impulse; other cases use an impulse followed by 9,999
noise samples and then silence. Noise uses xorshift32 (13,17,5), initialized
once to `0x7abeca01`, advanced before every sample across all cases, with
signed output scaled by `0.01f / float(INT32_MAX)`. Cases are ordered by
damping step, UnSolid selector, then UnReflect selector.

Node oracle binary format, little-endian:

- Magic `AWNODE01`, then a u32 case count.
- Each case: u8 damping step, u8 UnSolid, u8 UnReflect, zero reserved byte,
  u32 frame count.
- Each frame: one f32 input and eight expected f32 outputs.

Corpus SHA-256:
`6863a82d3f2111302b23463195bb5cb8d878007b3db9abb5dcd654233917806c`.

Both Rust implementations matched every output bit across block sizes
1, 16, 47, 64, 256 and 1024: **94,080,000 comparisons per implementation**.
That is 15,680,000 distinct reference output values checked in six block
configurations, not 94 million independent random values.
Additional Rust tests exercise parameter changes, reset/reuse, ring wraps,
empty/mismatched blocks, malformed files and zero heap allocations during
processing. A persistent Rust-only regression test regenerates the same
inputs and compares FNV-1a/64 digests against the C++ oracle's 75 output
streams, without requiring the external binary or a C++ compiler. These
digests are regression checks, not collision-free bitwise proofs. Neither
the corpus nor the tests prove equivalence for every floating-point input.

### Measured outcome

Apple M4 Pro, scalar arithmetic, same toolchains as above. Representative
256-frame results from the first final measurement pass:

| Implementation | Filters off, ns/sample | All filters on, ns/sample |
| --- | ---: | ---: |
| Extracted original C++ core | 12.934 | 14.591 |
| Samplewise Rust baseline | 8.423 | 11.105 |
| Borrowed-window Rust | 4.829 | 8.763 |

That is **2.68x / 1.67x throughput relative to the extracted C++ core**,
and **1.74x / 1.27x relative to the samplewise Rust baseline**. In the
reversed-order pass, borrowed Rust measured 5.030 / 9.009 ns/sample and
C++ 12.800 / 14.608 ns/sample: 2.54x / 1.62x. Ordinary run-to-run
variation exists; no CPU pinning or machine-wide isolation was used.

Assembly inspection found scalar arithmetic with neither packed arithmetic
nor FMA in either node. The optimized Rust inner sample loop has no
delay-array bounds checks; safe slice/window checks remain outside that
loop. Tone selector dispatch/control remains. A copied-tile prototype
was slower and was removed rather than presented as an improvement.

This supports a local, bit-identical scalar **reverb-core** improvement.
It does not establish a speedup for the entire plugin, MatrixVerb's complete
delay network, other CPUs, or any particular FMA-enabled build.

## Numerical backend interface

The borrowed-window processor is `Node<M: MatrixMath>`. `BorrowedNode`
selects `OriginalOrder`; `ApproximateNode` selects `Balanced`. Both use
identical state layouts and static dispatch. Balanced math computes
`-((a + b) + (c + d))` instead of `-(((a + b) + c) + d)` and therefore
does not pass the bit-exact contract. Only the original-order backend is
covered by the original C++ golden digests.

The root plugin runs separate live instances and crossfades the original-order
and approximate-math outputs; changing that mode does not reset or reinterpret
node memory. Acceleration changes arithmetic in those live nodes without
resetting their histories. Plugin integration is not included in the
core-only benchmark numbers above.

The node exposes a separate acceleration switch. Its enabled path uses
`f32::mul_add()` for the delay feedback operations, preserving scalar
processing while allowing targets with hardware FMA to use fused
instructions. FMA changes rounding and is not guaranteed to improve
throughput on every CPU. The four combinations are benchmarked by:

```sh
RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp" cargo run --release --bin node-bench
```
