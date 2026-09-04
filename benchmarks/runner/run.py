# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Run an isolated, shared-suite comparison; never commit or push results."""

import argparse
import sys
import traceback
from dataclasses import dataclass
from pathlib import Path

from .contracts import discover_specs
from .export import export_results
from .measurement import SuiteRunner
from .models import ComparisonResult, MeasurementIds, Profile
from .processes import execute
from .provenance import capture, write_json
from .report import render
from .suite import base_checkout, fingerprint


@dataclass(frozen=True)
class RunOptions:
    profile: Profile
    base: str | None
    output: Path


def parse_options() -> RunOptions:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=["smoke", "full"], default="smoke")
    parser.add_argument(
        "--base", help="target revision; compared at its merge base with HEAD"
    )
    parser.add_argument(
        "--output", type=Path, required=True, help="new output directory"
    )
    args = parser.parse_args()
    return RunOptions(args.profile, args.base, args.output.resolve())


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
    with base_checkout(suite, manifest.base_sha, manifest.suite_sha256) as base_suite:
        print("Building the same suite against the merge base", flush=True)
        baseline = SuiteRunner(base_suite, options.profile)
        base_status, base_rows = baseline.baseline(destination / "base", ids.vihaco)

        # Timing phases remain sequential to avoid contention between revisions.
        print("Measuring candidate", flush=True)
        candidate_rows = candidate.measure(candidate_dir, ids.vihaco, "vihaco")
        reference_dir = destination / "references"
        reference_dir.mkdir()
        print("Measuring shared native Rust and Python references", flush=True)
        references = candidate.measure(reference_dir, ids.references, "references")

    if fingerprint(suite) != manifest.suite_sha256:
        raise ValueError(
            "suite changed during measurement; rerun with unchanged sources"
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
