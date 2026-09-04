# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

import shutil
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import Mock, patch

from runner.contracts import discover, discover_specs, load_function, parse_case
from runner.estimates import collect, python_estimate
from runner.measurement import SuiteRunner
from runner.models import ComparisonResult, Estimate, Manifest, MeasurementIds
from runner.processes import execute
from runner.report import comparison, render
from runner.suite import fingerprint

WORKLOADS = Path(__file__).resolve().parents[2] / "workloads"


class ContractsTest(unittest.TestCase):
    def test_metadata_discovery_never_imports_workloads(self) -> None:
        with patch(
            "runner.contracts.load_function", side_effect=AssertionError("imported")
        ):
            self.assertTrue(discover_specs(WORKLOADS))

    def test_hanging_import_and_validation_are_supervised(self) -> None:
        for source in (
            "while True: pass\n",
            "def run(iterations, seed):\n    while True: pass\n",
        ):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory) / "workloads"
                bundle = root / "arithmetic"
                shutil.copytree(WORKLOADS / "arithmetic", bundle)
                (bundle / "python.py").write_text(source)
                self.assertTrue(discover_specs(root))
                with self.assertRaises(subprocess.TimeoutExpired):
                    execute(
                        [sys.executable, "-m", "runner.validate_python", str(root)],
                        WORKLOADS.parent,
                        Path(directory) / "validation.log",
                        timeout=1,
                        label="test validation",
                    )

    def test_workload_specific_limits_are_enforced_before_import(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / "recursive_fibonacci"
            shutil.copytree(WORKLOADS / "recursive_fibonacci", bundle)
            manifest = bundle / "workload.toml"
            manifest.write_text(
                manifest.read_text().replace(
                    "\niterations = 20\n", "\niterations = 21\n"
                )
            )
            with self.assertRaisesRegex(ValueError, "exceeds workload limits"):
                discover_specs(Path(directory))

    def test_all_python_implementations_match_expected_results(self) -> None:
        self.assertEqual(
            {item.id for item in discover(WORKLOADS)},
            {path.name for path in WORKLOADS.iterdir() if path.is_dir()},
        )

    def test_bundle_changes_invalidate_fingerprint(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / "arithmetic"
            shutil.copytree(WORKLOADS / "arithmetic", bundle)
            before = fingerprint(bundle)
            with (bundle / "native.rs").open("a") as stream:
                stream.write("\n// implementation changed\n")
            self.assertNotEqual(before, fingerprint(bundle))

    def test_wrong_expected_result_fails_before_measurement(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / "arithmetic"
            shutil.copytree(WORKLOADS / "arithmetic", bundle)
            manifest = bundle / "workload.toml"
            manifest.write_text(
                manifest.read_text().replace("expected = 7", "expected = 8", 1)
            )
            with self.assertRaisesRegex(ValueError, "Python mismatch"):
                discover(Path(directory))

    def test_duplicate_ids_and_missing_implementations_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copytree(WORKLOADS / "arithmetic", root / "one")
            shutil.copytree(WORKLOADS / "arithmetic", root / "two")
            with self.assertRaisesRegex(ValueError, "duplicate workload"):
                discover(root)
            (root / "one" / "python.py").unlink()
            with self.assertRaisesRegex(ValueError, "missing python.py"):
                discover(root)


class ReportTest(unittest.TestCase):
    def test_shared_references_are_not_revision_comparisons(self) -> None:
        def row(mean: float) -> Estimate:
            return Estimate(mean, mean * 0.9, mean * 1.1, 10, "test")

        manifest = Manifest(
            schema_version=2,
            run_id="test",
            reference_policy="shared",
            run_attempt="1",
            profile="smoke",
            head_sha="head",
            base_sha="base",
            suite_sha256="suite",
            workloads={},
            created_at="",
            environment={},
            dirty=False,
        )
        result = ComparisonResult(
            manifest,
            "available",
            {"a/short/sst/program": row(100)},
            {"a/short/sst/program": row(80)},
            {"a/short/rust/program": row(10), "a/short/python/program": row(200)},
        )
        report = render(result)
        comparisons, references = report.split("## Shared references")
        self.assertNotIn("/rust/program", comparisons)
        self.assertNotIn("/python/program", comparisons)
        self.assertIn("-20.0%", comparisons)
        self.assertIn("| a/short | 10.00× | 8.00× | 20.00× |", references)
        result = replace(result, base_status="unavailable", base={})
        self.assertIn("| a/short | — | 8.00× | 20.00× |", render(result))

    def test_collection_requires_expected_python_measurements(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(collect(Path(directory), []), {})
            with self.assertRaisesRegex(ValueError, "missing=.*python/program"):
                collect(Path(directory), ["a/short/python/program"])

    def test_overlapping_intervals_are_inconclusive(self) -> None:
        base = Estimate(100, 90, 110, 10, "test")
        candidate = Estimate(95, 85, 105, 10, "test")
        self.assertEqual(comparison(base, candidate)[1], "inconclusive")
        candidate = Estimate(120, 115, 125, 10, "test")
        self.assertEqual(comparison(base, candidate)[1], "possible slowdown")

    def test_python_uncertainty_uses_process_means(self) -> None:
        class Run:
            def __init__(self, values: list[float]) -> None:
                self.values = values

        class Benchmark:
            def get_runs(self) -> list[Run]:
                return [Run([]), Run([1, 3]), Run([4, 4])]

        estimate = python_estimate(Benchmark())
        self.assertEqual(estimate.samples, 2)
        self.assertEqual(estimate.mean_ns, 3e9)


class MeasurementTest(unittest.TestCase):
    @patch("runner.measurement.collect", return_value={})
    @patch("runner.measurement.execute")
    def test_references_run_once_and_revisions_only_measure_vihaco(
        self, execute: Mock, collect: Mock
    ) -> None:
        suite = WORKLOADS.parent
        runner = SuiteRunner(suite, "smoke")
        for revision in ("base", "candidate"):
            runner.measure(Path(revision), ["a/short/sst/program"], "vihaco")
        runner.measure(
            Path("references"),
            ["a/short/rust/program", "a/short/python/program"],
            "references",
        )
        self.assertEqual(execute.call_count, 4)
        for call, group, names in zip(
            execute.call_args_list[:3],
            ["vihaco", "vihaco", "references"],
            [
                ["a/short/sst/program"],
                ["a/short/sst/program"],
                ["a/short/rust/program"],
            ],
        ):
            self.assertEqual(call.args[3]["VIHACO_BENCH_GROUP"], group)
            self.assertEqual(call.kwargs["benchmarks"], names)
        python = execute.call_args_list[3]
        self.assertEqual(python.args[0][1:4], ["-u", "-m", "runner.python_bench"])
        self.assertEqual(python.kwargs["benchmarks"], ["a/short/python/program"])

    def test_expected_ids_separate_references_from_revision_measurements(self) -> None:
        workloads = discover(WORKLOADS)
        ids = MeasurementIds.for_workloads(workloads)
        cases = sum(len(workload.cases) for workload in workloads)
        self.assertEqual(len(ids.vihaco), 2 * cases + 3)
        self.assertEqual(len(ids.references), 2 * cases)
        self.assertTrue(all("/sst/" in name for name in ids.vihaco))
        self.assertFalse(set(ids.vihaco) & set(ids.references))

    @patch("runner.measurement.execute")
    def test_only_compile_failure_makes_base_unavailable(self, execute: Mock) -> None:
        import subprocess

        runner = SuiteRunner(WORKLOADS.parent, "smoke")
        with tempfile.TemporaryDirectory() as temporary:
            execute.side_effect = subprocess.CalledProcessError(1, ["cargo"])
            status, rows = runner.baseline(Path(temporary) / "compile", [])
            self.assertEqual((status, rows), ("unavailable", {}))
            execute.side_effect = [None, subprocess.CalledProcessError(1, ["cargo"])]
            with self.assertRaises(subprocess.CalledProcessError):
                runner.baseline(Path(temporary) / "validation", [])


class InputValidationTest(unittest.TestCase):
    @patch("runner.contracts.importlib.util.spec_from_file_location", return_value=None)
    def test_missing_import_spec_has_a_clear_error(self, spec: Mock) -> None:
        with self.assertRaisesRegex(ValueError, "cannot load Python"):
            load_function(Path("missing.py"), "missing")

    def test_bool_is_not_an_integer_input(self) -> None:
        with self.assertRaisesRegex(ValueError, "integer inputs"):
            parse_case(
                {"id": "case", "iterations": True, "seed": 1, "expected": 1}, set()
            )


if __name__ == "__main__":
    unittest.main()
