#!/usr/bin/env bash
# Generate the assembly of every example binary and build it into a ROM with RGBDS.
#
# Usage: scripts/assemble-examples.sh
# Needs rgbasm, rgblink and rgbfix (RGBDS >= 0.9) on PATH.
# Output: target/examples/<bin>/main.{asm,o,gb,sym,map} (under $CARGO_TARGET_DIR when it is set),
# each built by scripts/run.sh <bin>
set -euo pipefail

cd "$(dirname "$0")/.."

# Every example binary. Its directory, examples/<bin>/, holds its committed assembly
# (main.asm, the snapshot tests/snapshots.rs checks) and its INCBIN assets, if any: it goes on
# the include path.
examples=(basic_usage unbricked unbricked_std unbricked_rustboy fosdem coin-anim)

out_root="${CARGO_TARGET_DIR:-target}/examples"
failed=()

cargo build --quiet --bins

for bin in "${examples[@]}"; do
    if scripts/run.sh "$bin" > /dev/null; then
        echo "ok      $bin -> $out_root/$bin/main.gb"
    else
        echo "FAILED  $bin" >&2
        failed+=("$bin")
    fi
done

if ((${#failed[@]} > 0)); then
    echo "Failed examples: ${failed[*]}" >&2
    exit 1
fi
