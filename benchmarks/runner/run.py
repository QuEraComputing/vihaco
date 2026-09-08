# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Run an isolated, shared-suite comparison; never commit or push results."""

import argparse
import sys
import traceback
from dataclasses import dataclass, replace
from pathlib import Path

from .contracts import discover_specs
from .export import export_results
from .measurement import SuiteRunner
from .models import ComparisonResult, MeasurementIds, Profile
from .processes import execute, output
from .provenance import capture, write_json
from .report import render
from .suite import base_checkout, file_digest, fingerprint, harness_fingerprint


@dataclass(frozen=True)
class RunOptions:
    profile: Profile
    base: str | None
    output: Path
    base_machine: str | None = None
    base_machine_path: Path | None = None


def parse_options() -> RunOptions:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=["smoke", "full"], default="smoke")
    parser.add_argument(
        "--base", help="target revision; compared at its merge base with HEAD"
    )
    parser.add_argument(
        "--output", type=Path, required=True, help="new output directory"
    )
    overrides = parser.add_mutually_exclusive_group()
    overrides.add_argument(
        "--base-machine", help="revision supplying benchmarks/machine only"
    )
    overrides.add_argument(
        "--base-machine-path", type=Path, help="compatible machine crate directory"
    )
    args = parser.parse_args()
    return RunOptions(
        args.profile,
        args.base,
        args.output.resolve(),
        args.base_machine,
        args.base_machine_path,
    )


def run_comparison(suite: Path, options: RunOptions) -> ComparisonResult:
    destination = options.output
    destination.mkdir(parents=True, exist_ok=False)
    workloads = discover_specs(suite / "workloads")
    # Both import-time code and correctness checks belong behind the deadline.
    execute(
        [sys.executable, "-m", "runner.validate_python", str(suite / "workloads")],
        suite,
        destination / "discovery.log",
        timeout=120,
        label="Python validation",
    )
    ids = MeasurementIds.for_workloads(workloads)
    manifest = capture(suite, destination, options.profile, options.base, workloads)
    write_json(destination / "manifest.json", manifest)

    candidate_dir = destination / "candidate"
    candidate_dir.mkdir()
    candidate = SuiteRunner(suite, options.profile)
    print("Validating and building candidate", flush=True)
    candidate.validate(candidate_dir)
    candidate.build(candidate_dir)

    print("Preparing merge-base checkout", flush=True)
    machine_sha = (
        output(["git", "rev-parse", f"{options.base_machine}^{{commit}}"], suite.parent)
        if options.base_machine
        else None
    )
    with base_checkout(
        suite, manifest.base_sha, machine_sha, options.base_machine_path
    ) as base_suite:
        print("Building the same suite against the merge base", flush=True)
        baseline = SuiteRunner(base_suite, options.profile)
        base_status = baseline.prepare_baseline(destination / "base")
        manifest = replace(
            manifest,
            base_machine_sha=machine_sha
            or (None if options.base_machine_path else manifest.base_sha),
            base_machine_sha256=(
                fingerprint(base_suite / "machine")
                if (base_suite / "machine").is_dir()
                else None
            ),
            base_lock_sha256=file_digest(base_suite / "Cargo.lock"),
        )
        write_json(destination / "manifest.json", manifest)
        base_before = fingerprint(base_suite)
        base_rows = (
            baseline.measure(destination / "base", ids.vihaco, "vihaco")
            if base_status == "available"
            else {}
        )

        # Timing phases remain sequential to avoid contention between revisions.
        print("Measuring candidate", flush=True)
        candidate_rows = candidate.measure(candidate_dir, ids.vihaco, "vihaco")
        reference_dir = destination / "references"
        reference_dir.mkdir()
        print("Measuring shared native Rust and Python references", flush=True)
        references = candidate.measure(reference_dir, ids.references, "references")

        if fingerprint(base_suite) != base_before:
            raise ValueError("baseline changed during the comparison")

    if (
        harness_fingerprint(suite) != manifest.suite_sha256
        or fingerprint(suite / "machine") != manifest.machine_sha256
        or file_digest(suite / "Cargo.lock") != manifest.lock_sha256
    ):
        raise ValueError(
            "sources changed during the comparison; rerun with unchanged sources"
        )
    return ComparisonResult(
        manifest, base_status, base_rows, candidate_rows, references
    )


def main() -> None:
    options = parse_options()
    suite = Path(__file__).resolve().parents[1]
    existed = options.output.exists()
    try:
        result = run_comparison(suite, options)
        write_json(options.output / "results.json", result)
        (options.output / "report.md").write_text(render(result))
        export_results(options.output, result)
    except BaseException:  # noqa: BLE001 - CLI privacy boundary; details stay local.
        # Do not echo exception text, command arguments, or tracebacks into CI.
        # The directory may already exist or be unwritable; never overwrite a
        # previous run's diagnostics while reporting that failure.
        try:
            if not existed:
                with (options.output / "failure.log").open("x") as diagnostics:
                    traceback.print_exc(file=diagnostics)
        except OSError:
            pass
        print(
            "Benchmark failed. Review local diagnostics before sharing them.",
            flush=True,
        )
        raise SystemExit(1) from None
    print(
        "Benchmark complete. Reviewed exports are in the output's publish directory.",
        flush=True,
    )


if __name__ == "__main__":
    main()
