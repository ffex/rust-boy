#!/usr/bin/env bash
# Download the Blargg test ROMs that check the test emulator (tests/support/gameboy.rs).
#
# Usage: scripts/fetch-test-roms.sh <directory>
# Then:  GB_TEST_ROMS=<directory> cargo test --test emulator blargg
#
# The ROMs come from a fixed commit of github.com/retrio/gb-test-roms (a mirror of Blargg's
# test ROMs) and each one is checked against its SHA-256 below; a file already there with the
# right checksum is not downloaded again. They are test data, not part of this repository.
set -euo pipefail

if (($# != 1)); then
    echo "usage: $0 <directory>" >&2
    exit 2
fi
dir="$1"
commit="c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15"
base="https://raw.githubusercontent.com/retrio/gb-test-roms/$commit"

# <sha256>  <path>
roms=(
    "fe61349cbaee10cc384b50f356e541c90d1bc380185716706b5d8c465a03cf89  cpu_instrs/individual/01-special.gb"
    "fb90b0d2b9501910c49709abda1d8e70f757dc12020ebf8409a7779bbfd12229  cpu_instrs/individual/02-interrupts.gb"
    "ca553e606d9b9c86fbd318f1b916c6f0b9df0cf1774825d4361a3fdff2e5a136  cpu_instrs/individual/03-op sp,hl.gb"
    "7686aa7a39ef3d2520ec1037371b5f94dc283fbbfd0f5051d1f64d987bdd6671  cpu_instrs/individual/04-op r,imm.gb"
    "d504adfa0a4c4793436a154f14492f044d38b3c6db9efc44138f3c9ad138b775  cpu_instrs/individual/05-op rp.gb"
    "17ada54b0b9c1a33cd5429fce5b765e42392189ca36da96312222ffe309e7ed1  cpu_instrs/individual/06-ld r,r.gb"
    "ab31d3daaaa3a98bdbd9395b64f48c1bdaa889aba5b19dd5aaff4ec2a7d228a3  cpu_instrs/individual/07-jr,jp,call,ret,rst.gb"
    "974a71fe4c67f70f5cc6e98d4dc8c096057ff8a028b7bfa9f7a4330038cf8b7e  cpu_instrs/individual/08-misc instrs.gb"
    "b28e1be5cd95f22bd1ecacdd33c6f03e607d68870e31a47b15a0229033d5ba2a  cpu_instrs/individual/09-op r,r.gb"
    "7f5b8e488c6988b5aaba8c2a74529b7c180c55a58449d5ee89d606a07c53514a  cpu_instrs/individual/10-bit ops.gb"
    "0ec0cf9fda3f00becaefa476df6fb526c434abd9d4a4beac237c2c2692dac5d3  cpu_instrs/individual/11-op a,(hl).gb"
    "646067b3d6c79fda810e9c3f1cb7c0efd5abb0a7ac06437c54e65720c15d9925  instr_timing/instr_timing.gb"
    "52724532c5709e38e947eb429337c124c38bc68f373874435a7460548098b617  mem_timing/individual/01-read_timing.gb"
    "eea92d3f4e95aab5910e0f7080916a3c42a2b8deae1ee5d45d1e3751d648f3f6  mem_timing/individual/02-write_timing.gb"
    "2e9067c670ff8b45916bf321677ad04a6896d06a057dbcb82ae9f208a1ae9c34  mem_timing/individual/03-modify_timing.gb"
)

for entry in "${roms[@]}"; do
    sum="${entry%%  *}"
    path="${entry#*  }"
    file="$dir/$path"
    if [[ -f "$file" ]] && echo "$sum  $file" | sha256sum --check --status; then
        continue
    fi
    mkdir -p "$(dirname "$file")"
    url="$base/${path// /%20}"
    curl --fail --silent --show-error --location --retry 3 --output "$file" "$url"
    if ! echo "$sum  $file" | sha256sum --check --status; then
        echo "checksum mismatch: $path" >&2
        rm -f "$file"
        exit 1
    fi
    echo "fetched $path"
done
echo "Blargg test ROMs in $dir"
