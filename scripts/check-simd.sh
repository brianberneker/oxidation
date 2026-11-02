#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
directory="$root/target/simd-check"
mkdir -p "$directory"
bundles="${CARGO_TARGET_DIR:-$root/target}/bundled"

case "$(uname -s)" in
    Darwin)
        tool=otool
        ultralight="$bundles/oxidation.clap/Contents/MacOS/oxidation"
        matrixverb="$bundles/oxidation-matrixverb.clap/Contents/MacOS/oxidation-matrixverb"
        ;;
    Linux)
        tool=objdump
        ultralight="$bundles/oxidation.clap"
        matrixverb="$bundles/oxidation-matrixverb.clap"
        ;;
    *)
        printf '%s\n' "Error: SIMD disassembly check supports native macOS/Linux builds." >&2
        exit 1
        ;;
esac
if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'Error: SIMD disassembly check requires %s.\n' "$tool" >&2
    exit 1
fi

for plugin in ultralight matrixverb; do
    if [ "$plugin" = ultralight ]; then
        binary="$ultralight"
        pattern='vfmadd[0-9]*ps'
        if [ "$(uname -m)" = arm64 ] || [ "$(uname -m)" = aarch64 ]; then
            pattern='fmla[.]4s|fmla[[:space:]].*[.]4s'
        fi
    else
        binary="$matrixverb"
        pattern='vfmadd[0-9]*pd'
        if [ "$(uname -m)" = arm64 ] || [ "$(uname -m)" = aarch64 ]; then
            pattern='fmla[.]2d|fmla[[:space:]].*[.]2d'
        fi
    fi
    if [ ! -f "$binary" ]; then
        printf 'Error: build plugin bundles first; missing %s.\n' "$binary" >&2
        exit 1
    fi
    if [ "$tool" = otool ]; then
        otool -tvV "$binary" > "$directory/$plugin.txt"
    else
        objdump -d "$binary" > "$directory/$plugin.txt"
    fi
    if ! grep -Eq "$pattern" "$directory/$plugin.txt"; then
        printf 'Error: no packed FMA instruction found in %s.\n' "$binary" >&2
        exit 1
    fi
    printf 'PASS: packed SIMD/FMA instructions found in %s.\n' "$binary"
done
