# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Read timing-tool output and estimate uncertainty, independent of Markdown."""

import json
import math
import random
import statistics
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Protocol

import pyperf

from .models import Estimate, Measurements
from .validation import require_text


class MeasuredProcess(Protocol):
    @property
    def values(self) -> Sequence[float]: ...


class PythonBenchmark(Protocol):
    def get_runs(self) -> Sequence[MeasuredProcess]: ...


def python_estimate(benchmark: PythonBenchmark) -> Estimate:
    # Processes, not inner-loop iterations, are independent sampling units.
    # Calibration-only runs contain no values.
    means = [
        statistics.mean(run.values) * 1e9 for run in benchmark.get_runs() if run.values
    ]
    if len(means) < 2:
        raise ValueError("at least two measured Python processes required")
    rng = random.Random(0)
    bootstrap = sorted(
        statistics.mean(rng.choices(means, k=len(means))) for _ in range(2000)
    )
    return Estimate(
        statistics.mean(means),
        bootstrap[49],
        bootstrap[1949],
        len(means),
        "95% bootstrap interval of process means",
    )


def criterion_estimate(path: Path) -> tuple[str, Estimate]:
    identity = require_text(
        json.loads((path.parent / "benchmark.json").read_text())["full_id"]
    )
    mean = json.loads(path.read_text())["mean"]
    interval = mean["confidence_interval"]
    sample = json.loads((path.parent / "sample.json").read_text())
    estimate = Estimate(
        float(mean["point_estimate"]),
        float(interval["lower_bound"]),
        float(interval["upper_bound"]),
        len(sample["times"]),
        "95% Criterion bootstrap mean interval",
    )
    return identity, estimate


def add_estimate(rows: Measurements, identity: str, estimate: Estimate) -> None:
    if identity in rows:
        raise ValueError(f"duplicate measurement: {identity}")
    values = (estimate.mean_ns, estimate.lower_ns, estimate.upper_ns)
    if not all(math.isfinite(value) and value > 0 for value in values):
        raise ValueError("invalid timing estimate")
    rows[identity] = estimate


def collect(directory: Path, expected: Iterable[str]) -> Measurements:
    rows: Measurements = {}
    for path in sorted((directory / "criterion").rglob("new/estimates.json")):
        identity, estimate = criterion_estimate(path)
        add_estimate(rows, identity, estimate)
    python_path = directory / "python.json"
    if python_path.exists():
        for benchmark in pyperf.BenchmarkSuite.load(str(python_path)):
            add_estimate(rows, benchmark.get_name(), python_estimate(benchmark))
    expected_ids = set(expected)
    if set(rows) != expected_ids:
        raise ValueError(
            f"measurement IDs differ: missing={expected_ids - set(rows)}, "
            f"extra={set(rows) - expected_ids}"
        )
    return rows
