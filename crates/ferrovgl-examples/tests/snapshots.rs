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

#[test]
fn every_example_renders_and_matches_the_snapshot_table() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let table = read_table();
    let mut failures = Vec::new();
    let mut new_table = String::from(
        "# Rust-side snapshots of the ported LVGL examples: `name objects fnv1a64`.\n\
         # objects = descendants of the active screen after the example ran and\n\
         # 10 frames of 33 ms were rendered with the default theme; the hash covers\n\
         # the RGB bytes of the final 800x480 frame. Regenerate with\n\
         # UPDATE_SNAPSHOTS=1 cargo test -p ferrovgl-examples --test snapshots --locked\n",
    );

    for example in EXAMPLES {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut display = common::build(example);
            common::render_frames(&mut display);
            let screen = display.screen().expect("active screen");
            let objects = common::descendants(&display.ui, screen);
            let hash = common::frame_hash(&mut display);
            (objects, hash)
        }));
        let Ok((objects, hash)) = result else {
            failures.push(format!("{}: panicked", example.name));
            continue;
        };
        if objects == 0 {
            failures.push(format!("{}: created no objects", example.name));
        }
        writeln!(new_table, "{} {objects} {hash:016x}", example.name).unwrap();
        match table.get(example.name) {
            Some(&(exp_objects, exp_hash)) => {
                if exp_objects != objects {
                    failures.push(format!(
                        "{}: {objects} objects, the table expects {exp_objects}",
                        example.name
                    ));
                }
                if exp_hash != hash {
                    failures.push(format!(
                        "{}: frame hash {hash:016x}, the table expects {exp_hash:016x}",
                        example.name
                    ));
                }
            }
            None => failures.push(format!("{}: missing from tests/snapshots.txt", example.name)),
        }
    }

    for name in table.keys() {
        if !EXAMPLES.iter().any(|e| e.name == name) {
            failures.push(format!("{name}: in tests/snapshots.txt but not in EXAMPLES"));
        }
    }

    if update {
        std::fs::write(table_path(), new_table).expect("write tests/snapshots.txt");
        return;
    }
    assert!(
        failures.is_empty(),
        "{} snapshot mismatches (rerun with UPDATE_SNAPSHOTS=1 after an intended change):\n{}",
        failures.len(),
        failures.join("\n")
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
