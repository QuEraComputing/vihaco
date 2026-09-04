# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Build and measure one suite revision, keeping tool invocations consistent."""

import os
import subprocess
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from .estimates import collect
from .models import BaseStatus, MeasurementGroup, Measurements, Profile
from .processes import execute

BUILD_TIMEOUT_SECONDS = 1200
PYTHON_SMOKE_ARGUMENTS = [
    "--processes",
    "2",
    "--values",
    "2",
    "--warmups",
    "1",
    "--min-time",
    "0.01",
]


@dataclass(frozen=True)
class SuiteRunner:
    suite: Path
    profile: Profile

    def cargo(self, action: str, *arguments: str) -> list[str]:
        return [
            "cargo",
            action,
            "--locked",
            "--manifest-path",
            str(self.suite / "Cargo.toml"),
            *arguments,
        ]

    def validate(self, directory: Path) -> None:
        execute(
            self.cargo("test"),
            self.suite,
            directory / "validation.log",
            timeout=BUILD_TIMEOUT_SECONDS,
        )

    def build(self, directory: Path) -> None:
        execute(
            self.cargo("bench", "--bench", "workloads", "--no-run"),
            self.suite,
            directory / "build.log",
            timeout=BUILD_TIMEOUT_SECONDS,
        )

    def criterion(
        self,
        directory: Path,
        expected: Sequence[str],
        group: MeasurementGroup,
    ) -> None:
        env = dict(
            os.environ,
            VIHACO_BENCH_PROFILE=self.profile,
            VIHACO_BENCH_GROUP=group,
            VIHACO_CRITERION_OUTPUT=str(directory / "criterion"),
        )
        label = "native Rust" if group == "references" else "SST"
        execute(
            self.cargo("bench", "--bench", "workloads"),
            self.suite,
            directory / "criterion.log",
            env,
            label=f"{directory.name}: {label}",
            benchmarks=[name for name in expected if "/python/" not in name],
        )

    def python(self, directory: Path, expected: Sequence[str]) -> None:
        command = [
            sys.executable,
            "-u",
            "-m",
            "runner.python_bench",
            "-o",
            str(directory / "python.json"),
        ]
        if self.profile == "smoke":
            command.extend(PYTHON_SMOKE_ARGUMENTS)
        execute(
            command,
            self.suite,
            directory / "python.log",
            label=f"{directory.name}: Python",
            benchmarks=[name for name in expected if "/python/" in name],
        )

    def measure(
        self,
        directory: Path,
        expected: Sequence[str],
        group: MeasurementGroup,
    ) -> Measurements:
        self.criterion(directory, expected, group)
        if group == "references":
            self.python(directory, expected)
        return collect(directory, expected)

    def baseline(
        self,
        directory: Path,
        expected: Sequence[str],
    ) -> tuple[BaseStatus, Measurements]:
        directory.mkdir()
        # Only a compilation failure makes the base unavailable. Validation,
        # timeout, and measurement failures must still fail the comparison.
        try:
            self.build(directory)
        except subprocess.CalledProcessError:
            return "unavailable", {}
        self.validate(directory)
        print("Measuring base", flush=True)
        return "available", self.measure(directory, expected, "vihaco")
