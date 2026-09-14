# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Build the publication directory without copying tool metadata or logs."""

import json
from pathlib import Path

import pyperf

from .models import ComparisonResult
from .provenance import write_json
from .report import render
from .validation import require_list, require_number, require_table, require_text


def numbers(value: object) -> list[float]:
    values = [require_number(item) for item in require_list(value)]
    if any(item <= 0 for item in values):
        raise ValueError("samples must be positive")
    return values


def criterion_samples(directory: Path, expected: set[str]) -> dict[str, object]:
    samples: dict[str, object] = {}
    for path in sorted((directory / "criterion").rglob("new/sample.json")):
        identity = require_text(
            require_table(json.loads((path.parent / "benchmark.json").read_text()))[
                "full_id"
            ]
        )
        if identity not in expected or identity in samples:
            raise ValueError("unexpected or duplicate sample identity")
        raw = require_table(json.loads(path.read_text()))
        iterations, elapsed = numbers(raw["iters"]), numbers(raw["times"])
        if len(iterations) != len(elapsed) or not iterations:
            raise ValueError("invalid sample lengths")
        samples[identity] = {"iterations": iterations, "elapsed_ns": elapsed}
    if set(samples) != {name for name in expected if "/python/" not in name}:
        raise ValueError("missing Criterion samples")
    return samples


def python_samples(path: Path, expected: set[str]) -> dict[str, object]:
    samples: dict[str, object] = {}
    for benchmark in pyperf.BenchmarkSuite.load(str(path)):
        name = benchmark.get_name()
        if name not in expected or name in samples:
            raise ValueError("unexpected or duplicate Python sample identity")
        # Preserve process grouping but omit all pyperf machine metadata,
        # environment, command-line arguments, and calibration diagnostics.
        samples[name] = {
            "process_values_seconds": [
                numbers(list(run.values)) for run in benchmark.get_runs() if run.values
            ]
        }
    if set(samples) != {name for name in expected if "/python/" in name}:
        raise ValueError("missing Python samples")
    return samples


def export_results(directory: Path, result: ComparisonResult) -> None:
    samples: dict[str, object] = {}
    for group, rows in (
        ("base", result.base),
        ("candidate", result.candidate),
        ("references", result.references),
    ):
        expected = set(rows)
        collected = criterion_samples(directory / group, expected)
        if group == "references":
            collected.update(
                python_samples(directory / group / "python.json", expected)
            )
        samples[group] = collected
    destination = directory / "publish"
    destination.mkdir(exist_ok=False)
    write_json(destination / "manifest.json", result.manifest)
    write_json(destination / "results.json", result)
    (destination / "report.md").write_text(render(result))
    (destination / "samples.json").write_text(
        json.dumps({"schema_version": 1, "samples": samples}, indent=2) + "\n"
    )
