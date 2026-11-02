#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ]; then
    printf '%s\n' "Error: run this script on Linux, not macOS or Windows." >&2
    exit 1
fi

for tool in cargo rustc cc ar; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Error: missing %s. See the Linux quick start in README.md.\n' "$tool" >&2
        exit 1
    fi
done

host=$(rustc -vV | sed -n 's/^host: //p')
if [ "$host" != x86_64-unknown-linux-gnu ]; then
    printf 'Error: this script supports native x86_64 GNU/Linux; Rust host is %s.\n' "$host" >&2
    exit 1
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

# Keep Linux binaries separate when the checkout is shared with another OS.
export CARGO_TARGET_DIR="$root/target/linux"
export RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp"

printf '%s\n' "Building oxidation for $host (locked dependencies)"
rustc --version
cargo --version
cargo test --locked --release --workspace --target "$host"
cargo test --locked --release --manifest-path poc/matrix/Cargo.toml --target "$host"
cargo xtask bundle oxidation --release --locked --target "$host"
cargo xtask bundle oxidation-matrixverb --release --locked --target "$host"

bundles="$CARGO_TARGET_DIR/bundled"
if [ ! -f "$bundles/oxidation.clap" ] || \
   [ ! -d "$bundles/oxidation.vst3" ] || \
   [ ! -f "$bundles/oxidation-matrixverb.clap" ] || \
   [ ! -d "$bundles/oxidation-matrixverb.vst3" ]; then
    printf 'Error: expected CLAP and VST3 bundles were not found in %s.\n' "$bundles" >&2
    exit 1
fi

printf '\n%s\n' \
    "Build and automated tests passed. DAW testing is still required." \
    "CLAP: $bundles/oxidation.clap" \
    "VST3: $bundles/oxidation.vst3" \
    "MatrixVerb CLAP: $bundles/oxidation-matrixverb.clap" \
    "MatrixVerb VST3: $bundles/oxidation-matrixverb.vst3" \
    "Installation and test checklist: README.md, Linux quick start."
