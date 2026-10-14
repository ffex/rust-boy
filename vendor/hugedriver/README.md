# hUGEDriver

Unmodified files from [SuperDisk/hUGEDriver v6.1.1](https://github.com/SuperDisk/hUGEDriver/tree/v6.1.1),
commit `13a26513498637baee9997bd3cad9476224db723`. Upstream dedicates hUGEDriver to
the public domain; see the License section of `UPSTREAM_README.md`. This dedication
applies to these vendored files; rust-boy's Rust integration remains MIT licensed.

The Rust integration embeds the pinned files, expands their includes (using the
repository's hardware.inc), prefixes driver globals/macros, and replaces the WRAM
section with address constants into an engine-allocated 100-byte buffer. No upstream
file is edited. Preview and GBDK modes are disabled in this RGBDS integration.

Legacy EQU syntax is modernized to DEF/EQU for RGBDS >=0.9 during embedding.
`RustBoyMusicRamBase` is defined by the engine to the allocated buffer's numeric
address, avoiding forward-reference EQU expressions. Two additional engine control
bytes gate playback and distinguish pause from stop. `SHA256SUMS` verifies the
unmodified upstream files from the repository root with `shasum -a 256 -c`.
