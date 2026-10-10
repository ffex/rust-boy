//! Snapshot tests: the assembly each example prints is the committed
//! `examples/<bin>/main.asm`, line for line.
//!
//! A difference fails with a diff of the two. When the change is intended, write the new
//! output over the snapshots and commit them with the change:
//!
//! ```bash
//! UPDATE_SNAPSHOTS=1 cargo test --test snapshots
//! ```

mod support;

use support::examples::{EXAMPLES, example_asm, example_dir};

/// Lines of context around each change in a diff
const CONTEXT: usize = 3;
/// Diff lines shown at most; the rest is counted
const MAX_DIFF_LINES: usize = 200;

/// Check example `bin` against its snapshot, or write the snapshot with `UPDATE_SNAPSHOTS`
fn check_snapshot(bin: &str) {
    let path = example_dir(bin).join("main.asm");
    let actual = example_asm(bin);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        if std::fs::read_to_string(&path).ok().as_deref() != Some(actual.as_str()) {
            std::fs::create_dir_all(example_dir(bin)).expect("cannot create the example directory");
            std::fs::write(&path, &actual)
                .unwrap_or_else(|error| panic!("cannot write {}: {}", path.display(), error));
            eprintln!("updated {}", path.display());
        }
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read the snapshot {}: {}\nTo create it: UPDATE_SNAPSHOTS=1 cargo test --test snapshots",
            path.display(),
            error
        )
    });
    if let Some(diff) = diff(&expected, &actual) {
        panic!(
            "the assembly of `{}` differs from its snapshot {} (- snapshot, + output):\n\n{}\n\
             If the change is intended, update the snapshots and commit them:\n    \
             UPDATE_SNAPSHOTS=1 cargo test --test snapshots",
            bin,
            path.strip_prefix(support::examples::root())
                .unwrap_or(&path)
                .display(),
            diff
        );
    }
}

macro_rules! snapshot_tests {
    ($($name:ident: $bin:expr;)*) => {
        $(
            #[test]
            fn $name() {
                check_snapshot($bin);
            }
        )*

        /// Every example has its test above, and the three lists of examples agree: this
        /// file's, `support::examples::EXAMPLES` and `scripts/assemble-examples.sh`'s, and
        /// they are the binaries in `src/bin`
        #[test]
        fn every_example_has_a_snapshot_test() {
            assert_eq!([$($bin),*], EXAMPLES);
            let mut sorted = EXAMPLES.map(String::from).to_vec();
            sorted.sort();
            assert_eq!(sorted, binaries(), "EXAMPLES and the binaries in src/bin");
            assert_eq!(script_examples(), EXAMPLES, "the list in scripts/assemble-examples.sh");
        }
    };
}

snapshot_tests! {
    snapshot_basic_usage: "basic_usage";
    snapshot_unbricked: "unbricked";
    snapshot_unbricked_std: "unbricked_std";
    snapshot_unbricked_rustboy: "unbricked_rustboy";
    snapshot_fosdem: "fosdem";
    snapshot_coin_anim: "coin-anim";
}

/// The binaries in `src/bin`, sorted: `name.rs` files and `name/main.rs` directories
fn binaries() -> Vec<String> {
    let dir = support::examples::root().join("src/bin");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("src/bin")
        .map(|entry| entry.expect("a directory entry").path())
        .filter_map(|path| {
            if path.is_dir() && path.join("main.rs").is_file() {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                path.file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// The examples `scripts/assemble-examples.sh` builds: its `examples=(...)` line
fn script_examples() -> Vec<String> {
    let script =
        std::fs::read_to_string(support::examples::root().join("scripts/assemble-examples.sh"))
            .expect("scripts/assemble-examples.sh");
    let line = script
        .lines()
        .find_map(|line| line.strip_prefix("examples=("))
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("an `examples=(...)` line");
    line.split_whitespace().map(String::from).collect()
}

/// A diff of two texts, line by line, in the unified format (`@@ -a,b +c,d @@` hunks with
/// `CONTEXT` lines around each change), or `None` when they are equal. A missing final
/// newline counts as a difference.
fn diff(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let old: Vec<&str> = expected.split('\n').collect();
    let new: Vec<&str> = actual.split('\n').collect();
    let edits = edit_script(&old, &new);

    // Group the edits into hunks: changes closer than 2 * CONTEXT lines share one
    let changed: Vec<usize> = (0..edits.len())
        .filter(|&i| !matches!(edits[i], Edit::Same(..)))
        .collect();
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for &i in &changed {
        let start = i.saturating_sub(CONTEXT);
        let end = (i + CONTEXT + 1).min(edits.len());
        match hunks.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => hunks.push((start, end)),
        }
    }

    let mut out = Vec::new();
    for (start, end) in hunks {
        let (old_start, new_start) = edits[start].positions();
        let old_count = edits[start..end]
            .iter()
            .filter(|e| !matches!(e, Edit::Insert(..)))
            .count();
        let new_count = edits[start..end]
            .iter()
            .filter(|e| !matches!(e, Edit::Delete(..)))
            .count();
        out.push(format!(
            "@@ -{},{} +{},{} @@",
            old_start + 1,
            old_count,
            new_start + 1,
            new_count
        ));
        for edit in &edits[start..end] {
            out.push(match *edit {
                Edit::Same(i, _) => format!(" {}", old[i]),
                Edit::Delete(i, _) => format!("-{}", old[i]),
                Edit::Insert(_, j) => format!("+{}", new[j]),
            });
        }
    }
    let total = out.len();
    if total > MAX_DIFF_LINES {
        out.truncate(MAX_DIFF_LINES);
        out.push(format!(
            "... and {} more diff lines",
            total - MAX_DIFF_LINES
        ));
    }
    Some(out.join("\n"))
}

/// One step of an edit script: a line in both texts, only in the old one, or only in the
/// new one, with the positions in the old and new texts where it is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    Same(usize, usize),
    Delete(usize, usize),
    Insert(usize, usize),
}

impl Edit {
    fn positions(self) -> (usize, usize) {
        match self {
            Edit::Same(i, j) | Edit::Delete(i, j) | Edit::Insert(i, j) => (i, j),
        }
    }
}

/// The shortest edit script from `old` to `new`, from their longest common subsequence
/// (the snapshots are at most a few thousand lines, so the quadratic table is fine)
fn edit_script(old: &[&str], new: &[&str]) -> Vec<Edit> {
    let (n, m) = (old.len(), new.len());
    // lcs[i][j]: the longest common subsequence of old[i..] and new[j..]
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut edits = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            edits.push(Edit::Same(i, j));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            // A deletion first, so a changed line reads `-old` then `+new`
            edits.push(Edit::Delete(i, j));
            i += 1;
        } else {
            edits.push(Edit::Insert(i, j));
            j += 1;
        }
    }
    edits
}

#[test]
fn diff_of_equal_texts_is_none() {
    assert_eq!(diff("a\nb\n", "a\nb\n"), None);
}

/// A changed snapshot fails with a diff that names the line: the check the tests above
/// rely on, on a real snapshot with one instruction changed
#[test]
fn diff_shows_a_changed_line_with_its_context() {
    let snapshot = std::fs::read_to_string(example_dir("coin-anim").join("main.asm"))
        .expect("the coin-anim snapshot");
    let target = snapshot
        .lines()
        .position(|line| line.trim() == "call WaitVBlank")
        .expect("coin-anim waits for VBlank");
    let changed: String = snapshot
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == target {
                "    call WaitNotVBlank\n".to_string()
            } else {
                format!("{}\n", line)
            }
        })
        .collect();
    let diff = diff(&snapshot, &changed).expect("the texts differ");
    let lines: Vec<&str> = diff.lines().collect();
    assert_eq!(
        lines[0],
        format!("@@ -{},7 +{},7 @@", target - 2, target - 2),
        "{}",
        diff
    );
    assert!(lines.contains(&"-    call WaitVBlank"), "{}", diff);
    assert!(lines.contains(&"+    call WaitNotVBlank"), "{}", diff);
    assert_eq!(
        lines.len(),
        1 + 3 + 2 + 3,
        "one hunk, 3 lines of context: {}",
        diff
    );
}

#[test]
fn diff_shows_insertions_deletions_and_separate_hunks() {
    let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n";
    let new = "1\n2\nnew\n3\n4\n5\n6\n7\n8\n9\n10\n11\n13\n14\n15\n";
    let diff = diff(old, new).expect("the texts differ");
    assert_eq!(
        diff,
        "@@ -1,5 +1,6 @@\n 1\n 2\n+new\n 3\n 4\n 5\n\
         @@ -9,7 +10,6 @@\n 9\n 10\n 11\n-12\n 13\n 14\n 15"
    );
}
