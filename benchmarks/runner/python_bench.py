# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""pyperf controls process isolation, calibration, warmup, and sampling."""

from pathlib import Path

import pyperf

from .contracts import discover


def main() -> None:
    # pyperf must restart workers as a module too, preserving relative imports.
    runner = pyperf.Runner(program_args=("-m", "runner.python_bench"))
    root = Path(__file__).resolve().parents[1] / "workloads"
    for workload in discover(root):
        for case in workload.cases:
            # Keep the timed expression unchanged: timeit avoids an additional
            # Python wrapper call, which would distort the short cases.
            runner.timeit(
                f"{workload.id}/{case.id}/python/program",
                stmt="run(iterations, seed)",
                globals={
                    "run": workload.function,
                    "iterations": case.iterations,
                    "seed": case.seed,
                },
                metadata={"workload_sha256": workload.digest},
            )


if __name__ == "__main__":
    main()
