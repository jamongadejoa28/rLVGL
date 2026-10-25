//! Smoke and regression test of every ported example.
//!
//! Each example is built on the upstream-configured 800x480 test display with
//! the default theme, then [`common::FRAMES`] frames are rendered on the
//! deterministic clock. The resulting object count and a 64-bit FNV-1a hash of
//! the final frame are compared with `tests/snapshots.txt`.
//!
//! The hashes are Rust-side snapshots, not upstream reference images: they
//! make rendering or behaviour changes visible. After an intended change,
//! regenerate the table with
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p ferrovgl-examples --test snapshots --locked
//! ```
//!
//! and review the diff of `tests/snapshots.txt`.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use ferrovgl_examples::EXAMPLES;

fn table_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots.txt")
}

/// `name -> (object count, frame hash)` from `tests/snapshots.txt`.
fn read_table() -> BTreeMap<String, (u32, u64)> {
    let text = std::fs::read_to_string(table_path()).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next().expect("name").to_owned();
            let objects = parts.next().expect("object count").parse().expect("object count");
            let hash = u64::from_str_radix(parts.next().expect("hash"), 16).expect("hash");
            (name, (objects, hash))
        })
        .collect()
}

/// Header of `tests/snapshots.txt`.
const HEADER: &str = "# Rust-side snapshots of the ported LVGL examples: `name objects fnv1a64`.\n\
     # objects = descendants of the active screen after the example ran and\n\
     # 10 frames of 33 ms were rendered with the default theme; the hash covers\n\
     # the RGB bytes of the final 800x480 frame. Regenerate with\n\
     # UPDATE_SNAPSHOTS=1 cargo test -p ferrovgl-examples --test snapshots --locked\n";

/// The run of every example compared with the table.
#[derive(Debug, Default)]
struct Outcome {
    /// Broken examples (panics, empty scenes): failures in update mode too.
    errors: Vec<String>,
    /// Differences from the table: what `UPDATE_SNAPSHOTS=1` accepts.
    mismatches: Vec<String>,
    /// The table to write in update mode.
    new_table: String,
}

/// Compare `results` (`name -> (objects, hash)`, `None` when the example
/// panicked), in `EXAMPLES` order, with `table`.
fn tabulate(results: &[(&str, Option<(u32, u64)>)], table: &BTreeMap<String, (u32, u64)>) -> Outcome {
    let mut out = Outcome {
        new_table: String::from(HEADER),
        ..Outcome::default()
    };
    for &(name, result) in results {
        let Some((objects, hash)) = result else {
            out.errors.push(format!("{name}: panicked"));
            // Keep the previous row so an update does not drop the example.
            if let Some(&(exp_objects, exp_hash)) = table.get(name) {
                writeln!(out.new_table, "{name} {exp_objects} {exp_hash:016x}").unwrap();
            }
            continue;
        };
        if objects == 0 {
            out.errors.push(format!("{name}: created no objects"));
        }
        writeln!(out.new_table, "{name} {objects} {hash:016x}").unwrap();
        match table.get(name) {
            Some(&(exp_objects, exp_hash)) => {
                if exp_objects != objects {
                    out.mismatches.push(format!(
                        "{name}: {objects} objects, the table expects {exp_objects}"
                    ));
                }
                if exp_hash != hash {
                    out.mismatches.push(format!(
                        "{name}: frame hash {hash:016x}, the table expects {exp_hash:016x}"
                    ));
                }
            }
            None => out
                .mismatches
                .push(format!("{name}: missing from tests/snapshots.txt")),
        }
    }
    for name in table.keys() {
        if !results.iter().any(|&(n, _)| n == name) {
            out.mismatches
                .push(format!("{name}: in tests/snapshots.txt but not in EXAMPLES"));
        }
    }
    out
}

#[test]
fn every_example_renders_and_matches_the_snapshot_table() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let table = read_table();
    let results: Vec<(&str, Option<(u32, u64)>)> = EXAMPLES
        .iter()
        .map(|example| {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let mut display = common::build(example);
                common::render_frames(&mut display);
                let screen = display.screen().expect("active screen");
                let objects = common::descendants(&display.ui, screen);
                let hash = common::frame_hash(&mut display);
                (objects, hash)
            }));
            (example.name, result.ok())
        })
        .collect();
    let Outcome {
        mut errors,
        mismatches,
        new_table,
    } = tabulate(&results, &table);

    if update {
        std::fs::write(table_path(), new_table).expect("write tests/snapshots.txt");
        assert!(
            errors.is_empty(),
            "tests/snapshots.txt was written, but {} examples are broken (previous rows kept):\n{}",
            errors.len(),
            errors.join("\n")
        );
        return;
    }
    errors.extend(mismatches);
    assert!(
        errors.is_empty(),
        "{} snapshot mismatches (rerun with UPDATE_SNAPSHOTS=1 after an intended change):\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn a_broken_example_fails_the_update_and_keeps_its_row() {
    let table: BTreeMap<String, (u32, u64)> = [
        ("lv_example_a".to_owned(), (3, 0xa)),
        ("lv_example_b".to_owned(), (4, 0xb)),
        ("lv_example_gone".to_owned(), (1, 0xc)),
    ]
    .into_iter()
    .collect();
    let results = [
        ("lv_example_a", None),
        ("lv_example_b", Some((4, 0xbb))),
        ("lv_example_c", Some((0, 0xc))),
    ];
    let out = tabulate(&results, &table);
    assert_eq!(
        out.errors,
        ["lv_example_a: panicked", "lv_example_c: created no objects"]
    );
    assert_eq!(
        out.mismatches,
        [
            "lv_example_b: frame hash 00000000000000bb, the table expects 000000000000000b",
            "lv_example_c: missing from tests/snapshots.txt",
            "lv_example_gone: in tests/snapshots.txt but not in EXAMPLES",
        ]
    );
    let rows: Vec<&str> = out.new_table.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(
        rows,
        [
            "lv_example_a 3 000000000000000a",
            "lv_example_b 4 00000000000000bb",
            "lv_example_c 0 000000000000000c",
        ]
    );
}

#[test]
fn examples_are_unique_and_findable() {
    for (i, example) in EXAMPLES.iter().enumerate() {
        assert!(example.name.starts_with("lv_example_"), "{}", example.name);
        assert!(
            EXAMPLES[..i].iter().all(|e| e.name != example.name),
            "duplicate {}",
            example.name
        );
        let found = ferrovgl_examples::find(example.name).expect("find by full name");
        assert_eq!(found.name, example.name);
        let short = example.name.strip_prefix("lv_example_").unwrap();
        assert_eq!(ferrovgl_examples::find(short).map(|e| e.name), Some(example.name));
    }
    assert!(ferrovgl_examples::find("lv_example_does_not_exist").is_none());
}

#[test]
fn examples_can_run_twice_on_the_same_ui() {
    // C examples keep their styles in `static` variables and may be called
    // again; the second call must reuse them without panicking.
    for example in EXAMPLES {
        let mut display = common::build(example);
        display.wait(common::FRAME_MS);
        if let Some(screen) = display.screen() {
            display.ui.obj_clean(screen);
        }
        (example.run)(&mut display.ui);
        display.wait(common::FRAME_MS);
    }
}
