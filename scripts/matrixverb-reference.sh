#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
directory="$root/target/matrixverb-reference"
mkdir -p "$directory"
revision=7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a
source="$directory/MatrixVerbProc.cpp"
curl --fail --location --silent --show-error \
    "https://raw.githubusercontent.com/airwindows/airwindows/$revision/plugins/WinVST/MatrixVerb/MatrixVerbProc.cpp" \
    -o "$source"

"${CXX:-c++}" -std=c++17 -O3 -fno-fast-math -ffp-contract=off \
    -fno-tree-vectorize -fno-tree-slp-vectorize \
    -include "$root/crates/matrixverb-dsp/tests/reference/MatrixVerb.h" \
    "$source" "$root/crates/matrixverb-dsp/tests/reference/runner.cpp" \
    -o "$directory/matrixverb-reference"

export MATRIXVERB_REFERENCE="$directory/matrixverb-reference"
export RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp"
cargo test --locked --release --package oxidation-matrixverb-dsp --test reference -- --ignored --nocapture
"$MATRIXVERB_REFERENCE" bench
cargo run --locked --release --package oxidation-matrixverb-dsp --bin matrixverb-bench
