#!/usr/bin/env python3
"""Run locked, bounded cargo-fuzz campaigns from the workspace's uv environment."""

import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ("style_ops", "text_break", "tree_mutation")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=(*TARGETS, "all"), default="all", nargs="?")
    parser.add_argument("--seconds", type=int, default=30)
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    tool = ROOT / "target/tools/bin/cargo-fuzz"
    if not tool.is_file():
        parser.error("install cargo-fuzz 0.13.2 as documented in fuzz/README.md")
    version = subprocess.check_output([str(tool), "--version"], text=True).strip()
    if version != "cargo-fuzz 0.13.2":
        parser.error(f"expected cargo-fuzz 0.13.2, found {version}")
    env = dict(os.environ, RUSTUP_TOOLCHAIN="nightly")
    # cargo-fuzz has no --locked passthrough. Fetch/validate the existing lock
    # first, then forbid network access and verify the lock remains unchanged.
    subprocess.run(
        ["cargo", "fetch", "--locked", "--manifest-path", "fuzz/Cargo.toml"],
        cwd=ROOT, env=env, check=True,
    )
    lock = ROOT / "fuzz/Cargo.lock"
    before = lock.read_bytes()
    env["CARGO_NET_OFFLINE"] = "true"
    # Preserve the main workspace's incremental artifacts; only the instrumented
    # code uses target/fuzz. Line tables suffice for fuzz crash backtraces.
    selected = TARGETS if args.target == "all" else (args.target,)
    try:
        for target in selected:
            corpus = ROOT / "fuzz/corpus" / target
            corpus.mkdir(parents=True, exist_ok=True)
            print(f"Fuzzing {target} for {args.seconds} seconds", flush=True)
            subprocess.run(
                [str(tool), "fuzz", "run", "--target-dir", str(ROOT / "target/fuzz"),
                 "--codegen-units", "16", target, str(corpus), str(ROOT / "fuzz/seeds" / target),
                 "--", f"-max_total_time={args.seconds}", "-timeout=5", "-rss_limit_mb=512", "-max_len=1024"],
                cwd=ROOT, env=env, check=True,
            )
    finally:
        if lock.read_bytes() != before:
            raise RuntimeError("cargo-fuzz unexpectedly changed fuzz/Cargo.lock")


if __name__ == "__main__":
    main()
