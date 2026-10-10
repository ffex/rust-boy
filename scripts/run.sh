#!/usr/bin/env bash
# Build one example binary into a ROM, with its symbol and map files, and optionally run it.
#
# Usage: scripts/run.sh [--run | -r] [--emulator <command>] <bin>
#
#   <bin>               a binary of this crate: basic_usage, unbricked, unbricked_std,
#                       unbricked_rustboy, fosdem, coin-anim, or one of your own
#   --run, -r           open the ROM in an emulator, the first one found of:
#                       $GB_EMULATOR (a command, e.g. "wine ~/bgb/bgb.exe"), sameboy, mgba-qt, mgba,
#                       gambatte_qt, gambatte_sdl, bgb, bgb64, bgb.exe / bgb64.exe on the PATH through
#                       wine, pyboy; with none, it says so and prints where the files are
#   --emulator <cmd>    open it with <cmd> (implies --run)
#
# Output: target/examples/<bin>/main.{asm,o,gb,sym,map} (under $CARGO_TARGET_DIR when it is
# set). The .sym file sits next to the ROM with the same name, so emulators with a debugger
# (SameBoy, bgb, Emulicious) show the program's labels.
#
# Needs rgbasm, rgblink and rgbfix (RGBDS >= 0.9) on PATH. examples/<bin>/ (the example's
# assets) and include/ (hardware.inc) are on the include path.
set -euo pipefail

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//' >&2
    exit 2
}

run=false
emulator="${GB_EMULATOR:-}"
bin=""
while (($# > 0)); do
    case "$1" in
    -r | --run) run=true ;;
    --emulator)
        (($# >= 2)) || usage
        run=true
        emulator="$2"
        shift
        ;;
    -h | --help) usage ;;
    -*)
        echo "unknown option: $1" >&2
        usage
        ;;
    *)
        [[ -z "$bin" ]] || usage
        bin="$1"
        ;;
    esac
    shift
done
[[ -n "$bin" ]] || usage

cd "$(dirname "$0")/.."

out="${CARGO_TARGET_DIR:-target}/examples/$bin"
mkdir -p "$out"

include_flags=(-I include)
if [[ -d "examples/$bin" ]]; then
    include_flags+=(-I "examples/$bin")
fi

cargo run --quiet --bin "$bin" > "$out/main.asm"
rgbasm "${include_flags[@]}" -o "$out/main.o" "$out/main.asm"
rgblink -n "$out/main.sym" -m "$out/main.map" -o "$out/main.gb" "$out/main.o"
rgbfix -v -p 0xFF "$out/main.gb"

files() {
    echo "  ROM:     $out/main.gb"
    echo "  symbols: $out/main.sym"
    echo "  map:     $out/main.map"
    echo "  asm:     $out/main.asm"
}

if ! $run; then
    echo "built $bin"
    files
    exit 0
fi

# The first emulator found, as a command line (an array)
find_emulator() {
    local candidate
    for candidate in sameboy mgba-qt mgba gambatte_qt gambatte_sdl bgb bgb64; do
        if command -v "$candidate" > /dev/null; then
            command=("$candidate")
            return 0
        fi
    done
    if command -v wine > /dev/null; then
        for candidate in bgb.exe bgb64.exe; do
            if command -v "$candidate" > /dev/null; then
                command=(wine "$(command -v "$candidate")")
                return 0
            fi
        done
    fi
    if command -v pyboy > /dev/null; then
        command=(pyboy)
        return 0
    fi
    return 1
}

command=()
if [[ -n "$emulator" ]]; then
    # A command with its arguments, split on spaces
    read -r -a command <<< "$emulator"
elif ! find_emulator; then
    echo "built $bin; no emulator found (looked for \$GB_EMULATOR, sameboy, mgba-qt, mgba,"
    echo "gambatte_qt, gambatte_sdl, bgb, bgb64, bgb.exe / bgb64.exe through wine, pyboy):"
    files
    exit 0
fi

echo "built $bin; running ${command[*]} $out/main.gb"
files
exec "${command[@]}" "$out/main.gb"
