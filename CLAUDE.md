# CLAUDE.md

rust-boy is a Rust DSL that generates Game Boy assembly in RGBDS syntax (`cargo run` → `.asm` →
`rgbasm`/`rgblink`/`rgbfix` → `.gb`). It is being refactored:

- [`Task.md`](Task.md) — phased checklist.
- [`CONTEXT.md`](CONTEXT.md) — architecture, levels assessment, bug catalogue B1–B30 and branch analysis.
- [`AGENTS.md`](AGENTS.md) — the git/PR workflow: Codex's for its first try, and Claude's for the PRs it writes.

Read these before starting a task (reviewing, fixing or writing a PR), and keep them up to date in the same branch
as the change.

## Roles

- **Codex** makes the first implementation of a task: code, tests, docs, snapshots and a PR into
  `refactor`, following [`AGENTS.md`](AGENTS.md). After that first try, Codex hands the PR over.
- **Claude** owns the PR from then on: it has the PR reviewed, has Codex fix the findings on the PR
  branch (through a fix-request comment, below), has the new head reviewed again, and merges the PR
  into `refactor` when everything is OK.
- **The maintainer** is asked only for the final PR `refactor` → `main`, which Claude never merges,
  and for questions Claude cannot answer itself (see "Questions" below). The maintainer may also pass
  Codex's handoffs (a PR number) along; that is not a question.

## Working style

When a step doesn't need my input, keep going. Put status notes in the
same message as your next action.
Stop and ask only when you can't continue without me, or before anything
destructive: deleting data, force-pushing, or changing anything outside
this repository.

Once you have answered something, treat that answer as done. Focus on
what I'm asking now, and don't go back over an earlier answer unless I
ask about it or point out a problem with it.

Report findings with severity, file and line, failure scenario and evidence. Separate confirmed bugs
from questions and unverified concerns. State which checks ran and their results; never describe a
skipped check as passing.

## Commands

```bash
cargo build                               # library + all bins
cargo test                                # unit tests, doctests and the snapshot tests
UPDATE_SNAPSHOTS=1 cargo test --test snapshots   # rewrite examples/<bin>/main.asm after an intended
                                          # change of the generated asm; commit them with it
cargo clippy --all-targets                # lint
cargo fmt                                 # format
cargo run --bin <name> > main.asm         # bins: basic_usage, unbricked, unbricked_std,
                                          #       unbricked_rustboy, fosdem, coin-anim
scripts/run.sh <bin>                      # one bin -> target/examples/<bin>/main.{gb,sym,map,asm} via RGBDS
scripts/run.sh --run <bin>                # ... and open it in an emulator ($GB_EMULATOR, sameboy, mgba, gambatte,
                                          # bgb (also through wine), pyboy); none found: prints where the files are
scripts/assemble-examples.sh              # every example -> target/examples/<bin>/ with scripts/run.sh (needs RGBDS;
                                          # under $CARGO_TARGET_DIR when it is set)
RGBDS_LINK_CHECK=1 cargo test --lib --test emulator   # with RGBDS: link the test programs, and build ROMs
                                          # and run them on the test emulator (tests/emulator.rs)
scripts/fetch-test-roms.sh <dir>          # Blargg's test ROMs (pinned, checksummed); with GB_TEST_ROMS=<dir>,
                                          # tests/emulator.rs also checks the emulator against them
```

Headless-emulator tests (`tests/emulator.rs`): `support::rom::Rom::build(asm, include_dirs)` (or
`Rom::example(bin)`) assembles and links a program with RGBDS; `rom.boot()` gives a `support::gameboy::GameBoy`
(zero-dependency DMG emulator in `tests/support/gameboy.rs`) to run frames with scripted input
(`run_script(&[(frames, &[Button::Left])])`) and read WRAM/HRAM/VRAM/OAM (`read_symbol("wScore")`, `oam_entry(0)`),
the registers, and every OAM write with its PPU mode (`oam_writes()`, for [B12](CONTEXT.md#b12)). Without
`RGBDS_LINK_CHECK` these tests return early; CI runs them.

Each example's generated asm is committed as `examples/<bin>/main.asm`, next to its assets: the snapshot that
`tests/snapshots.rs` compares with the binary's output, which fails with a diff when they differ.

Assembling generated output (RGBDS ≥ 0.9). `include/hardware.inc` (v4.x) and the example's directory, for its
`.2bpp` assets, go on the include path with `-I`:

```bash
rgbasm -I include -I examples/fosdem -o main.o main.asm   # no `-L`: it was removed in RGBDS 0.8
rgblink -o main.gb main.o
rgbfix -v -p 0xFF main.gb
```

No RGBDS installed (e.g. in a cloud session)? Build it from the official source:
`git clone --depth 1 --branch v1.0.4 https://github.com/gbdev/rgbds` and run
`make rgbasm rgblink rgbfix` in that directory (needs a C++ compiler, bison and libpng).

## Architecture rules

- Layers: `asm` (instructions, `src/asm/`) → `stdlib` (stateless routines, `If`/flow control, `src/stdlib/`) →
  `engine` (`RustBoy` and its managers, `src/engine/`), with `hw` (pure data, `src/hw.rs`) beside them and
  `prelude` (re-exports). A layer depends only on the layers below it; `asm` does not use `hw` (see `CONTEXT.md`
  §3). Until Phase 2 they were `gb_asm`, `gb_std` and `rust_boy` (`rust_boy::rust_boy`).
- Every routine (Memcopy, WaitVBlank, UpdateKeys, …) exists **once**. Do not copy a routine into another layer.
  The exceptions, decided by the maintainer: the two examples written by hand with raw `asm` never link with the
  library's routines and keep their own: `src/bin/unbricked.rs`, the tutorial (WaitVBlank, Memcopy, UpdateKeys,
  GetTileByPixel), and `src/bin/basic_usage.rs`, the minimal example (WaitVBlank, which waits for `rLY` = 144).
- Generated output must be **deterministic**: never let `HashMap`/`HashSet` iteration order reach the
  output; use `BTreeMap` or an ordered `Vec`. Sprites, tiles, variables and functions are emitted in the
  order they were created.
- Generated labels must be unique and must not break RGBDS local-label scope (a global label inside an
  `If` body breaks `.end_if_N`). Prefer local labels or the label allocator.
- Do not hardcode hardware addresses/flags as strings in new code; use (or add to) the `hw` constants.
- Every bug fix gets a test; when it changes generated asm, the asm must still assemble.
- A change to an example's generated asm updates its snapshot `examples/<bin>/main.asm` in the same commit
  (`UPDATE_SNAPSHOTS=1 cargo test --test snapshots`); the PR shows the asm change in that diff.
- Never commit build artifacts (`*.gb`, `*.o`, `target/`).

## PR workflow

`refactor` is the integration branch: PRs target it (stacked PRs too, in their declared
order, with "Depends on #N"). The final release PR targets `main` from `refactor`. Never commit
directly to `main` or `refactor`; never force-push, rewrite published history or delete branches
without asking first. Do not mention or link AI sessions in commit messages or PR descriptions.

**Finding Codex's PRs.** Codex hands a PR over through the maintainer, who gives Claude its number,
or Claude picks up the open PRs into `refactor` whose *Independent review* section says "pending".
A PR Claude writes itself (a follow-up, a docs change) follows the git workflow of
[`AGENTS.md`](AGENTS.md) (branch names, small commits, ticking `Task.md`, `CHANGELOG.md`) but skips its
handoff: it starts at step 1 below with its *Independent review* section "pending".

For each PR (or chain of stacked PRs), Codex's or Claude's own:

1. **Independent review.** Start a reviewer agent with **no context** from the work, in its own
   worktree, read-only towards GitHub. Give it only the PR numbers, their branches and the merge
   order, not your own conclusions. It must:
   1. Record the base and PR head commit ids, simulate merging the PRs into `refactor` in order
      (never push), report conflicts, and record the commit id of the simulated merge.
   2. Run the CI checks from `.github/workflows/ci.yml` on the merged result: `cargo fmt --check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test`, `scripts/assemble-examples.sh`, and
      `RGBDS_LINK_CHECK=1 GB_TEST_ROMS=<dir> cargo test --lib --test emulator` after
      `scripts/fetch-test-roms.sh <dir>`; and `cargo +1.85 test` when Rust 1.85 is installed (CI's
      MSRV job). Report any check that could not run.
   3. Compare each PR description with the code it really changes; flag false or unverified claims,
      including claims about unchanged ROMs or generated asm.
   4. Review for bugs, regressions, missing regression tests and violations of the rules in this file.
   5. Check that `Task.md`, `CONTEXT.md` and, where relevant, `CHANGELOG.md` agree with the code, and
      that intentional generated-asm changes have matching snapshots that still assemble.
   6. Return the reviewed commit ids, conflicts, findings, checks and results, and open questions.
2. **Fix.** Claude posts the findings as one *fix-request* comment on the PR (format in
   [`AGENTS.md`](AGENTS.md#fix-requests)); the `@codex` mention starts Codex through its GitHub
   integration. Codex fixes every finding on the PR branch (merging `refactor` into it when it is
   behind; never rebasing a published branch), or writes why a finding is not fixed, and replies with
   the new head SHA. Claude fixes a trivial finding (a one-line or docs-only change) itself when a
   round-trip would cost more. Either way: a fix that changes behaviour updates `Task.md`,
   `CONTEXT.md` and `CHANGELOG.md` in the same branch and comes with a test; the CI checks run before
   every push; snapshots are updated only for an intended asm change, never to make a test pass.
   Claude finds Codex's reply by reading the PR's comments (`gh pr view N --comments`); if Codex
   has not answered when nothing else is left to do, Claude tells the maintainer the PR number.
   Reviewer agents may run on a smaller model (Sonnet) to save tokens.
3. **Re-review.** A review applies only to the commit ids it recorded: after fixes, start a new
   context-free review of the new head. Repeat 2–3 until the reviewer has no findings that block.
4. **Merge.** Merge only when CI is green on the reviewed head, there are no conflicts and no
   finding is left open without a written disposition. If `refactor` moved since the review, merge it
   into the PR branch and go back to step 3. Update the PR description (template
   below), including the *Independent review* section: what each review found and which commit
   fixed it. Then merge into `refactor` with a merge commit pinned to the reviewed head SHA (the
   merge tool's expected head SHA, or `gh pr merge N --merge --match-head-commit <sha>`), and check
   that CI on `refactor` is green; if it is red, fix it at once with a new PR through the same steps.
   **Stacked PRs.** A chain reviewed together is merged in its order, one pinned merge per PR,
   without merging `refactor` into the later branches: this replaces the rule above for the moves of
   `refactor` that the chain's own earlier merges make. Before each merge, check that `refactor` has
   moved only by those merges since the review; otherwise merge `refactor` in and go back to step 3.
   After the last merge, check that the tree of `refactor` is the reviewed one
   (`git diff <simulated merge commit> refactor` is empty).
   **Asking Codex.** Claude asks Codex for fixes only through fix-request comments on the PR. When
   Codex should redo a whole task, Claude says so to the maintainer.
5. **Release PR.** `refactor` → `main` is never merged by Claude: when it is ready, tell the
   maintainer and wait.

### Questions

Answer questions yourself when the code, `Task.md`, `CONTEXT.md` or an earlier decision of the
maintainer settles them, and write the answer in the PR. Ask the maintainer when a question is a
real design or product decision (API shape, a new dependency, a hardware target such as GBC, a
change of these rules), or before anything destructive. While waiting, keep working on other PRs.

### PR description template

```markdown
## Summary
Why this change: the problem and the outcome, in 2–4 sentences.

## What changed
- Per file or area, what was changed and why.

## Tasks closed
- Task.md items ticked, bug ids (e.g. B5, B7).

## How it was verified
- Commands run and their results (build, test, clippy, rgbasm/rgblink of affected examples).

## Generated-asm impact
- Which examples' output changes and how (attach a short diff excerpt if relevant), or "none".

## Risks and follow-ups
- Breaking changes, open questions, what is left for a later PR.

## Independent review
- What each context-free review found, the reviewed commit ids, and which commit fixed each finding
  (or why not), or "no findings".

## How to review
- Suggested reading order / what to focus on.
```
