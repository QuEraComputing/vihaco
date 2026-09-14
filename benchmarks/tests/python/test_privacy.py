# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

import contextlib
import io
import json
import os
import tempfile
import tomllib
import unittest
from dataclasses import asdict
from pathlib import Path
from unittest.mock import Mock, patch
from urllib.parse import urlparse

from runner.export import criterion_samples, export_results, python_samples
from runner.models import ComparisonResult, Estimate, Manifest
from runner.progress import Progress
from runner.provenance import build_flags_source, capture, environment
from runner.report import render
from runner.run import RunOptions, main

SENTINEL = "/private/company/SECRET_SENTINEL"
SUITE = Path(__file__).resolve().parents[2]


def manifest() -> Manifest:
    with (
        patch.dict(os.environ, {"RUSTFLAGS": SENTINEL}, clear=True),
        patch("runner.provenance.output", return_value="rustc 1.90.0 " + SENTINEL),
    ):
        metadata = environment(SUITE.parent)
    return Manifest(
        3,
        "123",
        "once per comparison; native Rust uses the candidate suite build",
        "1",
        "smoke",
        "a" * 40,
        "b" * 40,
        "c" * 64,
        {"example": "d" * 64},
        "2026-09-04T00:00:00+00:00",
        metadata,
        False,
    )


class PrivacyTest(unittest.TestCase):
    def test_capture_uses_opaque_local_identity(self) -> None:
        with (
            patch.dict(os.environ, {}, clear=True),
            patch("runner.provenance.output", return_value="a" * 40),
            patch("runner.provenance.environment", return_value={}),
        ):
            first = capture(SUITE, Path(SENTINEL), "smoke", None, [])
            second = capture(SUITE, Path(SENTINEL), "smoke", None, [])
        self.assertRegex(first.run_id, r"^local-[a-f0-9]{32}$")
        self.assertNotEqual(first.run_id, second.run_id)
        self.assertNotIn("SECRET_SENTINEL", json.dumps(asdict(first)))
        self.assertNotIn(
            "SECRET_SENTINEL",
            render(ComparisonResult(first, "unavailable", {}, {}, {})),
        )

    def test_capture_rejects_poisoned_ci_identity(self) -> None:
        for variables in (
            {"GITHUB_RUN_ID": SENTINEL},
            {"GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": SENTINEL},
            {"GITHUB_RUN_ID": "0"},
            {"GITHUB_RUN_ID": "1\n"},
            {"GITHUB_RUN_ID": "1" * 21},
            {"GITHUB_RUN_ATTEMPT": "1"},
        ):
            with patch.dict(os.environ, variables, clear=True):
                with self.assertRaises(ValueError) as failure:
                    capture(SUITE, Path(SENTINEL), "smoke", None, [])
                self.assertNotIn("SECRET_SENTINEL", str(failure.exception))

    def test_capture_retains_valid_ci_identity(self) -> None:
        with (
            patch.dict(
                os.environ,
                {"GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2"},
                clear=True,
            ),
            patch("runner.provenance.output", return_value="a" * 40),
            patch("runner.provenance.environment", return_value={}),
        ):
            result = capture(SUITE, Path(SENTINEL), "smoke", None, [])
        self.assertEqual((result.run_id, result.run_attempt), ("123", "2"))

    def test_flags_are_never_published_and_precedence_includes_empty_values(
        self,
    ) -> None:
        for variables, source in (
            ({}, "unset"),
            ({"RUSTFLAGS": SENTINEL}, "RUSTFLAGS"),
            ({"RUSTFLAGS": ""}, "RUSTFLAGS"),
            ({"CARGO_ENCODED_RUSTFLAGS": SENTINEL}, "CARGO_ENCODED_RUSTFLAGS"),
            (
                {"RUSTFLAGS": SENTINEL, "CARGO_ENCODED_RUSTFLAGS": ""},
                "CARGO_ENCODED_RUSTFLAGS",
            ),
        ):
            with (
                self.subTest(source=source, variables=list(variables)),
                patch.dict(os.environ, variables, clear=True),
                patch(
                    "runner.provenance.output", return_value="rustc 1.90.0 " + SENTINEL
                ),
                patch("runner.provenance.platform.system", return_value=SENTINEL),
                patch("runner.provenance.platform.machine", return_value=SENTINEL),
            ):
                self.assertEqual(build_flags_source(), source)
                encoded = json.dumps(environment(SUITE.parent))
                self.assertNotIn(SENTINEL, encoded)
                self.assertNotIn("sha256", encoded)
                self.assertIn(source, encoded)

    def test_progress_does_not_echo_build_output_or_measurement_suffixes(self) -> None:
        for expected in ([], ["example/short/sst/program"]):
            stream = io.StringIO()
            progress = Progress("test", expected, stream=stream)
            progress.observe(SENTINEL)
            progress.observe("Benchmarking example/short/sst/program: " + SENTINEL)
            progress.render("done")
            self.assertNotIn(SENTINEL, stream.getvalue())

    def test_criterion_export_keeps_only_numeric_samples(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sample = root / "criterion" / "example" / "new"
            sample.mkdir(parents=True)
            identity = "example/short/rust/program"
            (sample / "benchmark.json").write_text(
                json.dumps({"full_id": identity, "command": SENTINEL})
            )
            (sample / "sample.json").write_text(
                json.dumps({"iters": [1, 2], "times": [3, 6], "metadata": SENTINEL})
            )
            result = criterion_samples(root, {identity})
            self.assertEqual(
                result[identity], {"iterations": [1.0, 2.0], "elapsed_ns": [3.0, 6.0]}
            )
            self.assertNotIn(SENTINEL, json.dumps(result))

    def test_python_export_discards_all_tool_metadata(self) -> None:
        name = "example/short/python/program"
        benchmark = Mock()
        benchmark.get_name.return_value = name
        benchmark.get_runs.return_value = [
            Mock(values=[]),
            Mock(values=[0.1, 0.2], metadata=SENTINEL),
        ]
        with patch(
            "runner.export.pyperf.BenchmarkSuite.load", return_value=[benchmark]
        ):
            result = python_samples(Path("unused.json"), {name})
        self.assertEqual(result[name], {"process_values_seconds": [[0.1, 0.2]]})
        self.assertNotIn(SENTINEL, json.dumps(result))

    def test_exports_are_separate_from_raw_logs(self) -> None:
        estimate = Estimate(10, 9, 11, 2, "95% Criterion bootstrap mean interval")
        vm = {"example/short/sst/program": estimate}
        references = {
            "example/short/rust/program": estimate,
            "example/short/python/program": estimate,
        }
        result = ComparisonResult(manifest(), "available", vm, vm, references)
        self.assertNotIn(SENTINEL, json.dumps(asdict(result)))
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch("runner.export.criterion_samples", return_value={}),
            patch("runner.export.python_samples", return_value={}),
        ):
            root = Path(temporary)
            (root / "build.log").write_text(SENTINEL)
            export_results(root, result)
            self.assertEqual(
                {file.name for file in (root / "publish").iterdir()},
                {"manifest.json", "results.json", "report.md", "samples.json"},
            )
            for file in (root / "publish").iterdir():
                self.assertNotIn(SENTINEL, file.read_text())
            self.assertEqual((root / "build.log").read_text(), SENTINEL)

    def test_cli_failure_does_not_echo_exception_text(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "run"

            def fail(*args: object) -> None:
                destination.mkdir()
                raise ValueError(SENTINEL)

            stream = io.StringIO()
            with (
                patch(
                    "runner.run.parse_options",
                    return_value=RunOptions("smoke", None, destination),
                ),
                patch("runner.run.run_comparison", side_effect=fail),
                contextlib.redirect_stdout(stream),
                contextlib.redirect_stderr(stream),
                self.assertRaises(SystemExit),
            ):
                main()
            self.assertNotIn(SENTINEL, stream.getvalue())
            self.assertIn(SENTINEL, (destination / "failure.log").read_text())

    def test_lockfile_uses_only_public_package_hosts(self) -> None:
        lock = tomllib.loads((SUITE / "uv.lock").read_text())
        for package in lock["package"]:
            urls = [entry["url"] for entry in package.get("wheels", [])]
            if "sdist" in package:
                urls.append(package["sdist"]["url"])
            if "registry" in package["source"]:
                urls.append(package["source"]["registry"])
            for url in urls:
                self.assertIn(
                    urlparse(url).hostname, {"pypi.org", "files.pythonhosted.org"}
                )
                self.assertEqual(urlparse(url).scheme, "https")
                self.assertIsNone(urlparse(url).username)
                self.assertIsNone(urlparse(url).password)
