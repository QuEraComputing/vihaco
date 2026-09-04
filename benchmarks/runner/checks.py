# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Run CI quality checks with local-only diagnostics."""

import sys
from pathlib import Path

from .processes import execute


def main() -> None:
    suite = Path(__file__).resolve().parents[1]
    logs = suite.parent / "target" / "benchmark-checks"
    logs.mkdir(parents=True, exist_ok=True)
    commands = {
        "types": [
            sys.executable,
            "-m",
            "pyright",
            "--project",
            "../pyrightconfig.json",
        ],
        "lint": [sys.executable, "-m", "ruff", "check", "runner", "tests/python"],
        "format": [
            sys.executable,
            "-m",
            "ruff",
            "format",
            "--check",
            "runner",
            "tests/python",
        ],
        "rust-format": [
            "cargo",
            "fmt",
            "--manifest-path",
            "Cargo.toml",
            "--",
            "--check",
        ],
        "native-format": [
            "rustfmt",
            "--edition",
            "2024",
            "--check",
            *map(str, sorted((suite / "workloads").glob("*/native.rs"))),
        ],
        "clippy": [
            "cargo",
            "clippy",
            "--manifest-path",
            "Cargo.toml",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        "python-tests": [
            sys.executable,
            "-m",
            "unittest",
            "discover",
            "-s",
            "tests/python",
            "-p",
            "test_*.py",
        ],
        "ci-tests": ["node", "--test", "ci/tests/ci.test.cjs"],
    }
    try:
        for label, command in commands.items():
            execute(command, suite, logs / f"{label}.log", label=label)
    except BaseException:  # noqa: BLE001 - Never echo diagnostic exception text in CI.
        print(
            "Quality check failed; reproduce locally to review diagnostics.", flush=True
        )
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()
