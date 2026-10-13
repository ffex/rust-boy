#!/usr/bin/env bash
# Report linked ROM occupancy and observed frame-cycle budgets for one example, or all.
#
# Usage: scripts/report.sh [bin]
# Needs rgbasm, rgblink and rgbfix (RGBDS >= 0.9) on PATH.
set -euo pipefail

cd "$(dirname "$0")/.."

if (($# > 1)); then
    echo "usage: scripts/report.sh [bin]" >&2
    exit 2
fi

if (($# == 1)); then
    if [[ ! "$1" =~ ^[A-Za-z0-9_-]+$ ]]; then
        echo "not a binary name: $1" >&2
        exit 2
    fi
    export REPORT_BIN="$1"
fi

export RGBDS_LINK_CHECK=1
cargo test --quiet --test report rom_size_and_cycle_budget -- --ignored --nocapture
