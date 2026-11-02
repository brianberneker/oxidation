#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
directory="$root/target/ultralight-reference"
mkdir -p "$directory"
revision=7fd5f5cae262a41e6dd7d9a83d60c46e84d3092a
curl --fail --location --silent --show-error \
    "https://raw.githubusercontent.com/airwindows/airwindows/$revision/plugins/WinVST/Ultralight3/Ultralight3Proc.cpp" \
    -o "$directory/Ultralight3Proc.cpp"

{
    printf '%s\n' '#include "ultralight-node.h"' \
        'void UltralightNode::process(const float *input, std::array<float, 8> *output, std::size_t count, int step, int unSolid, int unReflect) {' \
        'const float damping = 0.0625f - float(step) * 0.003125f;' \
        'while (count--) { const float oQ = *input++;'
    awk '
        /aA\[cA\] = fA\*damping \+ oQ/ { extracting=1 }
        extracting && /if \(bypassVerb\)/ { exit }
        extracting { print }
    ' "$directory/Ultralight3Proc.cpp"
    printf '%s\n' '*output++ = {hA,hB,hC,hD,hE,hF,hG,hH};' '}}'
} > "$directory/core.cpp"
if ! grep -q 'switch (unReflect)' "$directory/core.cpp"; then
    printf '%s\n' "Error: upstream Ultralight3 extraction boundaries did not match." >&2
    exit 1
fi

"${CXX:-c++}" -std=c++17 -O3 -fno-fast-math -ffp-contract=off \
    -fno-tree-vectorize -fno-tree-slp-vectorize \
    -I "$root/poc/matrix/reference" \
    "$directory/core.cpp" "$root/poc/matrix/reference/ultralight-runner.cpp" \
    -o "$directory/ultralight-reference"

export RUSTFLAGS="-C no-vectorize-loops -C no-vectorize-slp"
"$directory/ultralight-reference" corpus > "$directory/reference.bin"
cargo run --locked --release --manifest-path poc/matrix/Cargo.toml --bin node-bench -- verify "$directory/reference.bin"
"$directory/ultralight-reference" bench
cargo run --locked --release --manifest-path poc/matrix/Cargo.toml --bin node-bench
