# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""The same harness compiles with incompatible checkout-specific machines."""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from runner.measurement import SuiteRunner
from runner.run import RunOptions, parse_options, run_comparison
from runner.suite import (
    IGNORED_DIRECTORIES,
    base_checkout,
    file_digest,
    fingerprint,
    harness_fingerprint,
)

SUITE = Path(__file__).resolve().parents[2]


def adapter(expression: str) -> str:
    return """
use vihaco_benchmark_api::{BenchmarkMachine, ConstantBenchmark, Result};
pub struct Machine;
impl BenchmarkMachine for Machine {
    type Program = ();
    type Invocation = u64;
    fn load(_: &str) -> Result<()> { Ok(()) }
    fn prepare(_: &(), _: u64, seed: u64) -> Result<u64> { Ok(seed) }
    fn execute<const COMPOSITE: bool>(state: &mut u64, _: &()) -> Result<u64> {
        Ok(EXPRESSION)
    }
}
impl<const ROUTE: u8> ConstantBenchmark<ROUTE> for Machine {
    type State = ();
    type Instruction = u64;
    fn prepare() -> Result<()> { Ok(()) }
    fn instruction(value: u64) -> u64 { value }
    fn execute(_: &mut (), value: &u64) -> Result<u64> { Ok(*value) }
}
""".replace("EXPRESSION", expression)


class CheckoutTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "hooks").mkdir()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init")
        (self.repo / "device/src").mkdir(parents=True)
        (self.repo / "device/Cargo.toml").write_text(
            '[package]\nname = "device"\nversion = "0.0.0"\nedition = "2024"\n'
        )
        self.device = self.repo / "device/src/lib.rs"
        self.device.write_text("pub fn old(value: u64) -> u64 { value }\n")
        self.no_machine = self.commit()
        self.suite = self.repo / "benchmarks"
        shutil.copytree(
            SUITE,
            self.suite,
            ignore=shutil.ignore_patterns(*IGNORED_DIRECTORIES, "machine"),
        )
        self.machine = self.suite / "machine"
        (self.machine / "src").mkdir(parents=True)
        (self.machine / "Cargo.toml").write_text("""[package]
name = "vihaco-benchmark-machine"
version = "0.0.0"
edition = "2024"
[dependencies]
vihaco-benchmark-api = { path = "../api" }
device = { path = "../../device" }
""")
        self.source = self.machine / "src/lib.rs"
        self.source.write_text(adapter("device::old(*state)"))
        self.driver = self.suite / "benches/workloads.rs"
        current_driver = self.driver.read_text()
        self.driver.write_text("This historical harness must never be compiled")
        self.old = self.commit()
        self.driver.write_text(current_driver)
        self.device.write_text(
            "pub struct Value(pub u64); impl Value { pub fn read(self) -> u64 { self.0 } }\n"
        )
        self.source.write_text(adapter("device::Value(*state).read()"))
        self.new = self.commit()

    def git(self, *arguments: str) -> str:
        return subprocess.check_output(
            [
                "git",
                "-c",
                "user.name=Benchmark Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                f"core.hooksPath={self.root / 'hooks'}",
                *arguments,
            ],
            cwd=self.repo,
            text=True,
            stderr=subprocess.PIPE,
        ).strip()

    def commit(self) -> str:
        self.git("add", ".")
        self.git("commit", "-m", "test snapshot")
        return self.git("rev-parse", "HEAD")

    def test_shared_harness_replaces_old_harness_but_preserves_old_machine(
        self,
    ) -> None:
        before = fingerprint(self.suite)
        with base_checkout(self.suite, self.old) as baseline:
            self.assertEqual(
                (baseline / "benches/workloads.rs").read_text(), self.driver.read_text()
            )
            self.assertIn("device::old", (baseline / "machine/src/lib.rs").read_text())
            self.assertIn(
                "pub fn old", (baseline.parent / "device/src/lib.rs").read_text()
            )
            self.assertEqual(
                harness_fingerprint(baseline), harness_fingerprint(self.suite)
            )
            self.assertNotEqual(
                fingerprint(baseline / "machine"), fingerprint(self.machine)
            )
        self.assertFalse(baseline.exists())
        self.assertEqual(fingerprint(self.suite), before)

    def test_identical_harness_compiles_against_breaking_library_apis(self) -> None:
        # This compiles the real harness, not a mock facade or copied timing code.
        env = dict(os.environ, CARGO_TARGET_DIR=str(self.root / "target"))
        for revision in (self.old, self.new):
            with (
                self.subTest(revision=revision),
                base_checkout(self.suite, revision) as baseline,
            ):
                command = [
                    "cargo",
                    "check",
                    "--offline",
                    "--manifest-path",
                    str(baseline / "Cargo.toml"),
                    "--workspace",
                    "--all-targets",
                ]
                result = subprocess.run(
                    command, env=env, capture_output=True, text=True, check=False
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    harness_fingerprint(baseline), harness_fingerprint(self.suite)
                )

    def test_overrides_select_only_machine_sources(self) -> None:
        for revision, path in ((self.old, None), (None, self.machine)):
            with base_checkout(self.suite, self.no_machine, revision, path) as baseline:
                self.assertIn(
                    "pub fn old", (baseline.parent / "device/src/lib.rs").read_text()
                )
                self.assertEqual(
                    harness_fingerprint(baseline), harness_fingerprint(self.suite)
                )
                expected = "device::old" if revision else "device::Value"
                self.assertIn(expected, (baseline / "machine/src/lib.rs").read_text())
        with (
            self.assertRaises(ValueError),
            base_checkout(self.suite, self.old, self.old, self.machine),
        ):
            self.fail("two overrides must fail")

    def test_missing_machine_is_unavailable_without_falling_back_to_candidate(
        self,
    ) -> None:
        with base_checkout(self.suite, self.no_machine) as baseline:
            self.assertFalse((baseline / "machine").exists())
            logs = self.root / "logs"
            self.assertEqual(
                SuiteRunner(baseline, "smoke").prepare_baseline(logs), "unavailable"
            )
            self.assertIn("--base-machine", (logs / "build.log").read_text())
        with (
            self.assertRaises(ValueError),
            base_checkout(self.suite, self.old, machine_path=self.root / "missing"),
        ):
            self.fail("invalid explicit override must fail")

    def test_baseline_lock_is_preserved_for_reconciliation(self) -> None:
        expected = (self.suite / "Cargo.lock").read_bytes()
        (self.suite / "Cargo.lock").write_text("new candidate dependency graph")
        with base_checkout(self.suite, self.old) as baseline:
            self.assertEqual((baseline / "Cargo.lock").read_bytes(), expected)
            self.assertEqual(
                harness_fingerprint(baseline), harness_fingerprint(self.suite)
            )

    def test_run_records_shared_harness_and_separate_machines(self) -> None:
        destination = self.root / "results"

        def resolve_lock(
            _command: object, suite: Path, *_args: object, **_kwargs: object
        ) -> None:
            (suite / "Cargo.lock").write_text("resolved baseline dependencies")

        with (
            patch("runner.run.execute"),
            patch("runner.measurement.execute", side_effect=resolve_lock),
            patch("runner.provenance.environment", return_value={}),
            patch.object(SuiteRunner, "validate"),
            patch.object(SuiteRunner, "build"),
            patch.object(SuiteRunner, "measure", return_value={}),
        ):
            result = run_comparison(
                self.suite, RunOptions("smoke", self.no_machine, destination, self.old)
            )
        manifest = result.manifest
        self.assertEqual(result.base_status, "available")
        self.assertEqual(manifest.base_sha, self.no_machine)
        self.assertEqual(manifest.base_machine_sha, self.old)
        self.assertEqual(manifest.suite_sha256, harness_fingerprint(self.suite))
        self.assertEqual(manifest.machine_sha256, fingerprint(self.machine))
        self.assertNotEqual(manifest.machine_sha256, manifest.base_machine_sha256)
        self.assertEqual(
            manifest.base_lock_sha256, file_digest(destination / "base/Cargo.lock")
        )
        self.assertEqual(
            json.loads((destination / "manifest.json").read_text())["machine_sha256"],
            manifest.machine_sha256,
        )

    def test_mutation_of_either_machine_or_shared_harness_aborts(self) -> None:
        for target in ("machine/src/lib.rs", "benches/workloads.rs", "Cargo.lock"):

            def mutate(
                runner: SuiteRunner, *_: object, path: str = target
            ) -> dict[str, object]:
                with (runner.suite / path).open("a") as stream:
                    stream.write("\n// changed during measurement\n")
                return {}

            with (
                self.subTest(target=target),
                patch("runner.run.execute"),
                patch("runner.measurement.execute"),
                patch("runner.provenance.environment", return_value={}),
                patch.object(SuiteRunner, "validate"),
                patch.object(SuiteRunner, "build"),
                patch.object(SuiteRunner, "measure", autospec=True, side_effect=mutate),
                self.assertRaisesRegex(ValueError, "changed during the comparison"),
            ):
                run_comparison(
                    self.suite,
                    RunOptions("smoke", self.old, self.root / target.replace("/", "-")),
                )


class OptionsTest(unittest.TestCase):
    def test_machine_override_does_not_select_library_revision(self) -> None:
        with patch(
            "sys.argv",
            [
                "runner",
                "--base",
                "main",
                "--base-machine",
                "old-machine",
                "--output",
                "results",
            ],
        ):
            options = parse_options()
        self.assertEqual(options.base, "main")
        self.assertEqual(options.base_machine, "old-machine")


if __name__ == "__main__":
    unittest.main()
