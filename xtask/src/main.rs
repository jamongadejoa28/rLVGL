//! Developer tasks: `cargo xtask <command>`.
//!
//! - `refimgs sync`: sparse-checkout LVGL v9.6.0 into `tests/upstream/lvgl`.
//! - `fonts [--check]`: convert upstream font C arrays into `.lvfnt` blobs.
//! - `nostd-check`: build the no_std crates for Cortex-M targets.
//! - `parity-report`: summarise the parity test results.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Upstream repository and pinned release.
const UPSTREAM_URL: &str = "https://github.com/lvgl/lvgl.git";
const UPSTREAM_TAG: &str = "v9.6.0";
const UPSTREAM_COMMIT: &str = "80ca777e37a2b176770726a02e07a6fb79ef0b39";
const UPSTREAM_DIR: &str = "tests/upstream/lvgl";
const SPARSE_PATHS: &[&str] = &[
    "src",
    "include",
    "tests/ref_imgs",
    "tests/src",
    "tests/configs",
    "docs/src/details/main-modules",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["refimgs", "sync"] => refimgs_sync(&root),
        ["fonts", rest @ ..] => fonts(&root, rest.contains(&"--check")),
        ["nostd-check"] => nostd_check(&root),
        ["parity-report"] => parity_report(&root),
        _ => {
            eprintln!("usage: cargo xtask <refimgs sync | fonts [--check] | nostd-check | parity-report>");
            Err("unknown command".into())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} failed with {status}").into())
    }
}

fn refimgs_sync(root: &Path) -> Result<()> {
    let dir = root.join(UPSTREAM_DIR);
    if !dir.join(".git").exists() {
        std::fs::create_dir_all(dir.parent().unwrap())?;
        run(Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--depth",
                "1",
                "--branch",
                UPSTREAM_TAG,
                "--filter=blob:none",
                "--sparse",
                UPSTREAM_URL,
            ])
            .arg(&dir))?;
    }
    run(Command::new("git")
        .current_dir(&dir)
        .args(["sparse-checkout", "set"])
        .args(SPARSE_PATHS))?;
    let head = Command::new("git")
        .current_dir(&dir)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if head != UPSTREAM_COMMIT {
        return Err(format!("upstream checkout is at {head}, expected {UPSTREAM_COMMIT}").into());
    }
    println!(
        "upstream lvgl {UPSTREAM_TAG} ({UPSTREAM_COMMIT}) ready in {}",
        dir.display()
    );
    Ok(())
}

fn fonts(root: &Path, check: bool) -> Result<()> {
    let upstream = root.join(UPSTREAM_DIR).join("src/font");
    let out_dir = root.join("crates/ferrovgl-fonts/data");
    std::fs::create_dir_all(&out_dir)?;
    let mut drift = Vec::new();
    for name in font_conv::BUILTIN_FONTS {
        let src = upstream.join(format!("lv_font_{name}.c"));
        let text = std::fs::read_to_string(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        let font = font_conv::parse_c_font(&text).map_err(|e| format!("{name}: {e}"))?;
        let blob = font_conv::encode(&font);
        let out = out_dir.join(format!("{name}.lvfnt"));
        let existing = std::fs::read(&out).ok();
        if existing.as_deref() == Some(blob.as_slice()) {
            continue;
        }
        if check {
            drift.push(name.to_string());
        } else {
            std::fs::write(&out, &blob)?;
            println!(
                "wrote {} ({} bytes, {} glyphs)",
                out.display(),
                blob.len(),
                font.glyphs.len()
            );
        }
    }
    if !drift.is_empty() {
        return Err(format!("font blobs out of date: {}", drift.join(", ")).into());
    }
    Ok(())
}

fn nostd_check(root: &Path) -> Result<()> {
    for target in ["thumbv7em-none-eabihf", "thumbv6m-none-eabi"] {
        for (pkg, features) in [
            (
                "ferrovgl-core",
                "float,flex,grid,snapshot,observer,bidi,arabic-persian-chars",
            ),
            ("ferrovgl-draw-sw", "complex,cf-rgb565,cf-xrgb8888"),
            ("ferrovgl-fonts", "montserrat-14,dejavu-16-persian-hebrew"),
            (
                "ferrovgl-widgets",
                "label,button,image,bar,slider,switch,checkbox,arc,line,led,spinner,buttonmatrix,textarea,observer,bidi,arabic-persian-chars,theme-default,theme-simple",
            ),
        ] {
            run(Command::new("cargo").current_dir(root).args([
                "build",
                "--locked",
                "--release",
                "--target",
                target,
                "-p",
                pkg,
                "--no-default-features",
                "--features",
                features,
            ]))?;
        }
    }
    Ok(())
}

fn parity_report(root: &Path) -> Result<()> {
    let dir = root.join("target/parity");
    let mut results = Vec::new();
    collect_parity_results(&dir, &dir, &mut results)?;
    results.sort();
    println!("## Pixel parity\n");
    if results.is_empty() {
        println!("No upstream image comparisons recorded (0 matched, 0 run). Pixel parity is unverified.");
        return Ok(());
    }
    let matched = results.iter().filter(|(_, status)| status == "matched").count();
    let mismatched = results.iter().filter(|(_, status)| status == "mismatch").count();
    let errors = results.len() - matched - mismatched;
    println!(
        "{matched}/{} recorded comparisons matched; {mismatched} mismatched; {errors} errors.\n",
        results.len()
    );
    for (name, status) in &results {
        println!("- {name}: {status}");
    }
    println!(
        "\nThese are the latest stored results in `target/parity`, not proof that every scenario ran in this invocation. Remove that directory before a fresh parity run."
    );
    Ok(())
}

fn collect_parity_results(root: &Path, dir: &Path, results: &mut Vec<(String, String)>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_parity_results(root, &path, results)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "result") {
            let text = std::fs::read_to_string(&path)?;
            let status = match text.lines().next() {
                Some("matched") => "matched",
                Some("mismatch") => "mismatch",
                _ => "error",
            };
            let name = path.strip_prefix(root)?.with_extension("");
            results.push((name.to_string_lossy().into_owned(), status.to_owned()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_results_recurse_and_do_not_treat_absent_diffs_as_success() {
        let dir = std::env::temp_dir().join(format!("ferrovgl-parity-report-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("widgets")).unwrap();
        std::fs::write(dir.join("widgets/label.result"), "matched\n").unwrap();
        std::fs::write(dir.join("mismatch.result"), "mismatch\n").unwrap();
        std::fs::write(dir.join("missing-reference.result"), "error\nmissing reference\n").unwrap();
        std::fs::write(dir.join("truncated.result"), "").unwrap();
        std::fs::write(dir.join("old_diff.png"), "").unwrap();
        let mut results = Vec::new();
        collect_parity_results(&dir, &dir, &mut results).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(
            results.iter().filter(|(_, status)| status == "matched").count(),
            1
        );
        assert_eq!(
            results.iter().filter(|(_, status)| status == "mismatch").count(),
            1
        );
        assert_eq!(results.iter().filter(|(_, status)| status == "error").count(), 2);
        assert!(
            results
                .iter()
                .any(|(name, _)| Path::new(name) == Path::new("widgets/label"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
