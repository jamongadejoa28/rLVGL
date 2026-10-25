//! Developer tasks: `cargo xtask <command>`.
//!
//! - `refimgs sync`: sparse-checkout LVGL v9.6.0 into `tests/upstream/lvgl`.
//! - `fonts [--check]`: convert upstream font C arrays into `.lvfnt` blobs.
//! - `nostd-check`: build the no_std crates for Cortex-M targets.
//! - `parity-report`: summarise the parity test results and the in-scope coverage
//!   recorded in `crates/ferrovgl-test/parity_scope.toml`.
//! - `gen-mapping [--check]`: regenerate `docs/src/mapping.md` (LVGL C API -> ferrovgl).
//! - `size-report [--update-baseline]`: measure `.text` of the no_std size probe for Cortex-M targets.

mod mapping;
mod size;

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
    "examples",
    "demos/render",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["refimgs", "sync"] => refimgs_sync(&root),
        ["fonts", rest @ ..] => fonts(&root, rest.contains(&"--check")),
        ["nostd-check"] => nostd_check(&root),
        ["parity-report"] => parity_report(&root),
        ["gen-mapping", rest @ ..] => mapping::gen_mapping(&root, rest.contains(&"--check")),
        ["size-report", rest @ ..] => size::size_report(&root, rest),
        _ => {
            eprintln!(
                "usage: cargo xtask <refimgs sync | fonts [--check] | nostd-check | parity-report | gen-mapping [--check] | size-report [--update-baseline]>"
            );
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
                "float,flex,grid,snapshot,observer,bidi,arabic-persian-chars,obj-id",
            ),
            ("ferrovgl-draw-sw", "complex,cf-rgb565,cf-xrgb8888"),
            ("ferrovgl-fonts", "montserrat-14,dejavu-16-persian-hebrew"),
            (
                "ferrovgl-widgets",
                "label,button,image,bar,slider,switch,checkbox,arc,line,led,spinner,buttonmatrix,textarea,observer,float,bidi,arabic-persian-chars,theme-default,theme-simple",
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
    scope_coverage(root, &results)
}

/// Scope table that classifies every upstream reference image.
const PARITY_SCOPE: &str = "crates/ferrovgl-test/parity_scope.toml";

/// One `[[image]]` entry of the parity scope table.
#[derive(Debug, Default, PartialEq)]
struct ScopeImage {
    path: String,
    test_file: String,
    test_fn: String,
    group: String,
    in_scope: bool,
    matched: bool,
    reason: Option<String>,
}

/// Print in-scope coverage: live `target/parity` results joined with the scope table.
fn scope_coverage(root: &Path, results: &[(String, String)]) -> Result<()> {
    let scope_path = root.join(PARITY_SCOPE);
    println!("\n## In-scope coverage\n");
    let text = match std::fs::read_to_string(&scope_path) {
        Ok(text) => text,
        Err(e) => {
            println!("`{PARITY_SCOPE}` not readable ({e}); in-scope coverage unknown.");
            return Ok(());
        }
    };
    let images = parse_scope(&text)?;
    let live = |image: &ScopeImage| {
        let name = image.path.strip_suffix(".png").unwrap_or(&image.path);
        results
            .iter()
            .find(|(result, _)| Path::new(result) == Path::new(name))
            .map(|(_, status)| status.as_str())
    };
    let in_scope: Vec<&ScopeImage> = images.iter().filter(|image| image.in_scope).collect();
    let unmatched: Vec<(&ScopeImage, &str)> = in_scope
        .iter()
        .filter_map(|image| match live(image) {
            Some("matched") => None,
            Some(status) => Some((*image, status)),
            None => Some((*image, "not run")),
        })
        .collect();
    let out_matched = images
        .iter()
        .filter(|image| !image.in_scope && live(image) == Some("matched"))
        .count();
    println!(
        "{}/{} in-scope reference images matched ({} images in scope table, {} out of scope, {out_matched} of those matched anyway).\n",
        in_scope.len() - unmatched.len(),
        in_scope.len(),
        images.len(),
        images.len() - in_scope.len(),
    );
    if unmatched.is_empty() {
        println!("No in-scope image is unmatched.");
    } else {
        println!("In-scope images not matched:\n");
        for (image, status) in &unmatched {
            println!(
                "- {} ({}::{}, group {}): {status}; reason: {}",
                image.path,
                image.test_file,
                image.test_fn,
                image.group,
                image.reason.as_deref().unwrap_or("none recorded")
            );
        }
    }
    let unknown: Vec<&str> = results
        .iter()
        .filter(|(name, _)| {
            !images.iter().any(|image| {
                Path::new(image.path.strip_suffix(".png").unwrap_or(&image.path)) == Path::new(name)
            })
        })
        .map(|(name, _)| name.as_str())
        .collect();
    if !unknown.is_empty() {
        println!(
            "\nResults with no entry in `{PARITY_SCOPE}`: {}",
            unknown.join(", ")
        );
    }
    let stale: Vec<&str> = images
        .iter()
        .filter(|image| image.matched != (live(image) == Some("matched")))
        .map(|image| image.path.as_str())
        .collect();
    if !stale.is_empty() {
        const SHOWN: usize = 10;
        let more = stale.len().saturating_sub(SHOWN);
        println!(
            "\n`{PARITY_SCOPE}` is stale: its `matched` flag differs from these results for {} images ({}{}). Update the table after a fresh parity run.",
            stale.len(),
            stale[..stale.len().min(SHOWN)].join(", "),
            if more > 0 {
                format!(", and {more} more")
            } else {
                String::new()
            }
        );
    }
    Ok(())
}

/// Parse the `[[image]]` tables of the scope table (the subset of TOML it uses:
/// basic strings, booleans and single-line arrays, which are skipped).
fn parse_scope(text: &str) -> Result<Vec<ScopeImage>> {
    let mut images = Vec::new();
    let mut current: Option<ScopeImage> = None;
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            images.extend(current.take());
            if line == "[[image]]" {
                current = Some(ScopeImage::default());
            }
            continue;
        }
        let Some(image) = current.as_mut() else {
            continue;
        };
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("{PARITY_SCOPE}:{}: expected `key = value`", number + 1))?;
        let value = value.trim();
        let string = || {
            parse_basic_string(value)
                .ok_or_else(|| format!("{PARITY_SCOPE}:{}: expected a string", number + 1))
        };
        let boolean = || match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("{PARITY_SCOPE}:{}: expected a boolean", number + 1)),
        };
        match key.trim() {
            "path" => image.path = string()?,
            "test_file" => image.test_file = string()?,
            "test_fn" => image.test_fn = string()?,
            "group" => image.group = string()?,
            "reason" => image.reason = Some(string()?),
            "in_scope" => image.in_scope = boolean()?,
            "matched" => image.matched = boolean()?,
            _ => {}
        }
    }
    images.extend(current);
    if let Some(image) = images.iter().find(|image| image.path.is_empty()) {
        return Err(format!("{PARITY_SCOPE}: an [[image]] has no path ({image:?})").into());
    }
    Ok(images)
}

/// Decode a TOML basic string (`"..."` with backslash escapes).
fn parse_basic_string(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    Some(out)
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

    #[test]
    fn scope_table_parses_images_and_skips_summary() {
        let text = r#"
# comment
[summary]
all = 2
[[summary.group]]
key = "g"
test_files = ["a.c"]

[[image]]
path = "widgets/a.png"
test_file = "a.c"
test_fn = "test_a"
also_test_fns = ["test_b"]
group = "g"
in_scope = true
matched = false
reason = "needs \"A:\" fs"

[[image]]
path = "b.png"
test_file = "b.c"
test_fn = "test_b"
group = "out"
in_scope = false
matched = true
"#;
        let images = parse_scope(text).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].path, "widgets/a.png");
        assert!(images[0].in_scope && !images[0].matched);
        assert_eq!(images[0].reason.as_deref(), Some("needs \"A:\" fs"));
        assert!(!images[1].in_scope && images[1].matched && images[1].reason.is_none());
        assert!(parse_scope("[[image]]\nin_scope = maybe\n").is_err());
    }

    #[test]
    fn real_scope_table_parses() {
        let root = workspace_root();
        let text = std::fs::read_to_string(root.join(PARITY_SCOPE)).unwrap();
        let images = parse_scope(&text).unwrap();
        assert!(!images.is_empty());
        assert!(
            images
                .iter()
                .all(|image| !image.test_file.is_empty() && !image.group.is_empty())
        );
    }
}
