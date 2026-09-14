# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Review native scaling and disassembly without imposing a timing CI gate."""

import argparse
import json
import re
import subprocess
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from .contracts import discover_specs
from .estimates import collect
from .models import Estimate, MeasurementIds, WorkloadCase, WorkloadSpec
from .validation import require_table, require_text


@dataclass(frozen=True)
class Scaling:
    """Interval bounds are a conservative ratio range, not a ratio CI."""

    mean_ratio: float
    lower_ratio: float
    upper_ratio: float

    @classmethod
    def compare(cls, short: Estimate, long: Estimate) -> "Scaling":
        return cls(
            long.mean_ns / short.mean_ns,
            long.lower_ns / short.upper_ns,
            long.upper_ns / short.lower_ns,
        )

    @property
    def assessment(self) -> str:
        if self.lower_ratio > 1:
            return "growth observed; inspect assembly"
        return "REVIEW: growth not established"


def scaling_cases(workload: WorkloadSpec) -> tuple[WorkloadCase, WorkloadCase]:
    cases = {case.id: case for case in workload.cases}
    short, long = cases["short"], cases["long"]
    if short.seed != long.seed or not 0 < short.iterations < long.iterations:
        raise ValueError(f"{workload.id}: scaling needs same-seed, increasing inputs")
    return short, long


def verify_workloads(run: Path, workloads: Sequence[WorkloadSpec]) -> None:
    manifest = require_table(json.loads((run / "manifest.json").read_text()))
    recorded = require_table(manifest["workloads"])
    actual = {workload.id: workload.digest for workload in workloads}
    if {key: require_text(value) for key, value in recorded.items()} != actual:
        raise ValueError("run workload fingerprints differ; measure the current suite")


def scaling_report(run: Path, workloads: Sequence[WorkloadSpec]) -> str:
    verify_workloads(run, workloads)
    rows = collect(
        run / "references", MeasurementIds.for_workloads(workloads).references
    )
    lines = [
        "Native scaling (long / short, with the same seed)",
        "Growth is supporting evidence, not proof that the intended work survives.",
        "",
    ]
    for workload in workloads:
        short, long = scaling_cases(workload)
        prefix = workload.id
        scaling = Scaling.compare(
            rows[f"{prefix}/{short.id}/rust/program"],
            rows[f"{prefix}/{long.id}/rust/program"],
        )
        lines.append(
            f"{prefix}: input {short.iterations} -> {long.iterations}, "
            f"time {scaling.mean_ratio:.2f}x "
            f"(interval-bound range {scaling.lower_ratio:.2f}.."
            f"{scaling.upper_ratio:.2f}x); {scaling.assessment}"
        )
    return "\n".join(lines)


def native_assembly(disassembly: str, count: int) -> str:
    """Extract native modules and helpers from GNU/LLVM demangled objdump text.

    Do not count instructions or infer control flow from architecture-specific
    mnemonics. Human review must follow calls and look for the intended work.
    """
    header = re.compile(r"^\s*[0-9a-fA-F]+ <(.+)>:\s*$")
    module = re.compile(r"vihaco_benchmark::workloads::workload_(\d+)::")
    found: set[int] = set()
    selected: list[str] = []
    keep = False
    for line in disassembly.splitlines():
        if match := header.match(line):
            keep = False
            if native := module.search(match[1]):
                keep = True
                found.add(int(native[1]))
        if keep:
            selected.append(line)
    if found != set(range(count)):
        raise ValueError(
            "native symbols missing or unexpected; check binary, debug symbols, "
            "objdump demangling, and generated native registry"
        )
    return "\n".join(selected)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path, help="completed comparison output directory")
    parser.add_argument("--binary", type=Path, help="the measured Criterion executable")
    parser.add_argument("--objdump", default="objdump", help="GNU or LLVM objdump")
    args = parser.parse_args()
    suite = Path(__file__).resolve().parents[1]
    workloads = discover_specs(suite / "workloads")
    print(scaling_report(args.run, workloads))
    if args.binary:
        # This reads the executable; it does not run it. Use the exact measured
        # build, not an independently compiled copy with different settings.
        result = subprocess.run(
            [args.objdump, "--disassemble", "--demangle", str(args.binary.resolve())],
            check=True,
            capture_output=True,
            text=True,
        )
        print("\nNative module mapping (sorted bundle directories):")
        for index, workload in enumerate(workloads):
            print(f"workload_{index}: {workload.id}")
        print("\n" + native_assembly(result.stdout, len(workloads)))


if __name__ == "__main__":
    main()
