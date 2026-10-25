//! `cargo xtask size-report [--update-baseline]`: runs the host tests of
//! the probe's allocator, builds the no_std size probe in
//! `tools/nostd-size` in release mode for the Cortex-M targets,
//! prints its `.text` / `.rodata` / `.data` / `.bss` sizes (measured with
//! `llvm-size` from the rustup `llvm-tools` component) and compares them
//! with `tools/nostd-size/baseline.toml`.
//!
//! The check fails if `.text` of any target grew by more than
//! [`MAX_TEXT_GROWTH_PERCENT`] over the baseline. `--update-baseline`
//! rewrites the baseline with the measured sizes instead.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Result, run};

/// Targets the probe is built for, in report order.
const TARGETS: &[&str] = &["thumbv7em-none-eabihf", "thumbv6m-none-eabi"];
/// Probe package directory relative to the workspace root.
const PROBE_DIR: &str = "tools/nostd-size";
/// Name of the probe binary.
const PROBE_BIN: &str = "ferrovgl-nostd-size";
/// Baseline file relative to [`PROBE_DIR`].
const BASELINE_FILE: &str = "baseline.toml";
/// Sections reported (and stored in the baseline), in report order.
const SECTIONS: &[&str] = &["text", "rodata", "data", "bss"];
/// Allowed `.text` growth over the baseline before the check fails.
const MAX_TEXT_GROWTH_PERCENT: u64 = 5;

/// Section sizes in bytes, keyed by section name without the dot.
type Sizes = BTreeMap<String, u64>;

/// Builds the size probe and prints (or checks) its section sizes.
pub fn size_report(root: &Path, args: &[&str]) -> Result<()> {
    let mut update = false;
    for arg in args {
        match *arg {
            "--update-baseline" => update = true,
            other => return Err(format!("size-report: unknown argument `{other}`").into()),
        }
    }

    let probe = root.join(PROBE_DIR);
    let llvm_size = llvm_size()?;
    let mut measured = BTreeMap::new();
    let target_dir = probe.join("target");
    // The probe's allocator lives in its library so it can be tested on the
    // host before the firmware is measured.
    run(probe_cargo(&probe)
        .args(["test", "--locked", "--lib", "--target-dir"])
        .arg(&target_dir))?;
    for target in TARGETS {
        run(probe_cargo(&probe)
            .args([
                "build",
                "--locked",
                "--release",
                "--target",
                target,
                "--target-dir",
            ])
            .arg(&target_dir))?;
        let elf = target_dir.join(target).join("release").join(PROBE_BIN);
        measured.insert((*target).to_string(), section_sizes(&llvm_size, &elf)?);
    }

    let baseline_path = probe.join(BASELINE_FILE);
    let baseline = if baseline_path.exists() {
        parse_baseline(&std::fs::read_to_string(&baseline_path)?)?
    } else {
        BTreeMap::new()
    };

    println!("## no_std size probe (`{PROBE_DIR}`, release)\n");
    println!("| target | section | bytes | baseline | delta |");
    println!("|---|---|---:|---:|---:|");
    let mut failures = Vec::new();
    for target in TARGETS {
        let sizes = &measured[*target];
        let base = baseline.get(*target);
        for section in SECTIONS {
            let now = sizes[*section];
            let (base_cell, delta_cell) = match base.and_then(|b| b.get(*section)) {
                Some(&was) => (was.to_string(), delta(now, was)),
                None => ("-".to_string(), "-".to_string()),
            };
            println!("| {target} | .{section} | {now} | {base_cell} | {delta_cell} |");
        }
        match base.and_then(|b| b.get("text")) {
            Some(&was) if sizes["text"] * 100 > was * (100 + MAX_TEXT_GROWTH_PERCENT) => {
                failures.push(format!(
                    "{target}: .text grew from {was} to {} bytes (more than {MAX_TEXT_GROWTH_PERCENT}%)",
                    sizes["text"]
                ))
            }
            Some(_) => {}
            None if !update => failures.push(format!("{target}: no baseline in {}", baseline_path.display())),
            None => {}
        }
    }

    if update {
        std::fs::write(&baseline_path, render_baseline(&measured)?)?;
        println!("\nbaseline written to {}", baseline_path.display());
        return Ok(());
    }
    if failures.is_empty() {
        println!("\n.text is within {MAX_TEXT_GROWTH_PERCENT}% of the baseline for every target.");
        Ok(())
    } else {
        for f in &failures {
            eprintln!("size-report: {f}");
        }
        Err(format!(
            "size check failed; if the growth is intended run `cargo xtask size-report --update-baseline` and commit {PROBE_DIR}/{BASELINE_FILE}"
        )
        .into())
    }
}

/// A `cargo` command in the probe package with the build-affecting
/// environment removed (see [`leaks_into_probe`]).
fn probe_cargo(probe: &Path) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(probe);
    for (key, _) in std::env::vars_os() {
        if key.to_str().is_some_and(leaks_into_probe) {
            cmd.env_remove(key);
        }
    }
    cmd
}

/// Whether the environment variable `key` would change where or how the
/// probe is built. `cargo xtask` may run under a caller's cargo
/// configuration; flags, target directory, profile or per-target
/// overrides from it must not reach the standalone probe build, or the
/// measured ELF would be stale (built elsewhere) or built differently.
/// Covers both spellings: `RUSTFLAGS` / `CARGO_TARGET_DIR` and their
/// `CARGO_BUILD_*` config equivalents, plus `CARGO_TARGET_<triple>_*` and
/// `CARGO_PROFILE_*`.
fn leaks_into_probe(key: &str) -> bool {
    matches!(key, "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS")
        || ["CARGO_BUILD_", "CARGO_TARGET_", "CARGO_PROFILE_"]
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// Signed change of `now` relative to `was`, e.g. `+1234 (+0.61%)`.
fn delta(now: u64, was: u64) -> String {
    let diff = now as i128 - was as i128;
    if was == 0 {
        return format!("{diff:+}");
    }
    format!("{diff:+} ({:+.2}%)", diff as f64 * 100.0 / was as f64)
}

/// `$(rustc --print sysroot)/lib/rustlib/<host>/bin/llvm-size`.
fn llvm_size() -> Result<PathBuf> {
    let sysroot = command_stdout(Command::new("rustc").args(["--print", "sysroot"]))?;
    let version = command_stdout(Command::new("rustc").arg("-vV"))?;
    let host = version
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .ok_or("`rustc -vV` did not print the host triple")?;
    let exe = if cfg!(windows) {
        "llvm-size.exe"
    } else {
        "llvm-size"
    };
    let path = Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host.trim())
        .join("bin")
        .join(exe);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "{} not found; install it with `rustup component add llvm-tools`",
            path.display()
        )
        .into())
    }
}

/// Run `cmd` and return its standard output.
fn command_stdout(cmd: &mut Command) -> Result<String> {
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(format!("{cmd:?} failed with {}", out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// Sizes of [`SECTIONS`] in `elf`, from `llvm-size -A` (sysv format).
fn section_sizes(llvm_size: &Path, elf: &Path) -> Result<Sizes> {
    let text = command_stdout(Command::new(llvm_size).arg("-A").arg(elf))?;
    let mut sizes = Sizes::new();
    for line in text.lines() {
        let mut cols = line.split_whitespace();
        let (Some(name), Some(size)) = (cols.next(), cols.next()) else {
            continue;
        };
        let Some(name) = name.strip_prefix('.') else {
            continue;
        };
        if SECTIONS.contains(&name) {
            *sizes.entry(name.to_string()).or_default() += size.parse::<u64>()?;
        }
    }
    for section in SECTIONS {
        // A section the linker did not emit (e.g. no `.data`) is empty.
        sizes.entry((*section).to_string()).or_default();
    }
    Ok(sizes)
}

/// Parse the baseline: `[target]` tables of `section = bytes` integers.
fn parse_baseline(text: &str) -> Result<BTreeMap<String, Sizes>> {
    let mut out: BTreeMap<String, Sizes> = BTreeMap::new();
    let mut current: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim().trim_matches('"').to_string();
            out.entry(name.clone()).or_default();
            current = Some(name);
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("{BASELINE_FILE}:{}: expected `key = value`", n + 1))?;
        let table = current
            .as_ref()
            .ok_or_else(|| format!("{BASELINE_FILE}:{}: key outside of a [target] table", n + 1))?;
        let value = value
            .trim()
            .replace('_', "")
            .parse::<u64>()
            .map_err(|e| format!("{BASELINE_FILE}:{}: {e}", n + 1))?;
        out.get_mut(table)
            .expect("table inserted above")
            .insert(key.trim().to_string(), value);
    }
    Ok(out)
}

/// Render the baseline file for `measured`.
fn render_baseline(measured: &BTreeMap<String, Sizes>) -> Result<String> {
    let rustc = command_stdout(Command::new("rustc").arg("--version"))?;
    let mut s = String::new();
    writeln!(
        s,
        "# Section sizes in bytes of the no_std size probe (tools/nostd-size),"
    )?;
    writeln!(
        s,
        "# release profile. Written by `cargo xtask size-report --update-baseline`;"
    )?;
    writeln!(
        s,
        "# `cargo xtask size-report` fails if .text grows more than {MAX_TEXT_GROWTH_PERCENT}%."
    )?;
    writeln!(s, "# Measured with {}", rustc.trim())?;
    for target in TARGETS {
        writeln!(s, "\n[{target}]")?;
        for section in SECTIONS {
            writeln!(s, "{section} = {}", measured[*target][*section])?;
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_roundtrip_ignores_comments() {
        let text =
            "# c\n[thumbv6m-none-eabi]\ntext = 1_000 # x\nbss = 4\n\n[\"thumbv7em-none-eabihf\"]\ntext = 9\n";
        let b = parse_baseline(text).unwrap();
        assert_eq!(b["thumbv6m-none-eabi"]["text"], 1000);
        assert_eq!(b["thumbv6m-none-eabi"]["bss"], 4);
        assert_eq!(b["thumbv7em-none-eabihf"]["text"], 9);
        assert!(parse_baseline("text = 1").is_err());
        assert!(parse_baseline("[t]\ntext = x").is_err());
    }

    #[test]
    fn build_environment_is_scrubbed() {
        for key in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_TARGET_DIR",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_THUMBV6M_NONE_EABI_RUSTFLAGS",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
        ] {
            assert!(leaks_into_probe(key), "{key}");
        }
        for key in [
            "CARGO_HOME",
            "CARGO_PKG_NAME",
            "PATH",
            "RUSTUP_TOOLCHAIN",
            "CARGO_NET_OFFLINE",
        ] {
            assert!(!leaks_into_probe(key), "{key}");
        }
    }

    #[test]
    fn delta_formats_sign_and_percent() {
        assert_eq!(delta(105, 100), "+5 (+5.00%)");
        assert_eq!(delta(90, 100), "-10 (-10.00%)");
        assert_eq!(delta(3, 0), "+3");
    }
}
