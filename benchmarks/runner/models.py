# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Typed data shared by discovery, measurement, and reporting.

Dataclasses describe validated data in memory. asdict() preserves the existing
JSON artifact schema; paths and executable functions stay out of result models.
"""

from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Literal

from .validation import require_integer

Profile = Literal["smoke", "full"]
MeasurementGroup = Literal["vihaco", "references"]
BaseStatus = Literal["available", "unavailable"]
WorkloadFunction = Callable[[int, int], int]


@dataclass(frozen=True)
class WorkloadCase:
    id: str
    iterations: int
    seed: int
    expected: int


@dataclass(frozen=True)
class WorkloadSpec:
    id: str
    algorithm: str
    numeric_constraints: str
    cases: tuple[WorkloadCase, ...]
    digest: str


@dataclass(frozen=True)
class Workload(WorkloadSpec):
    function: WorkloadFunction

    def validate(self) -> None:
        """Execute correctness checks outside all timing regions."""
        for case in self.cases:
            actual = self.function(case.iterations, case.seed)
            message = (
                f"Python mismatch: {self.id}/{case.id}: {actual} != {case.expected}"
            )
            try:
                result = require_integer(actual)
            except ValueError as error:
                raise ValueError(message) from error
            if result != case.expected:
                raise ValueError(message)


@dataclass(frozen=True)
class MeasurementIds:
    vihaco: tuple[str, ...]
    references: tuple[str, ...]

    @classmethod
    def for_workloads(cls, workloads: Sequence[WorkloadSpec]) -> "MeasurementIds":
        prefixes = [
            f"{workload.id}/{case.id}"
            for workload in workloads
            for case in workload.cases
        ]
        vihaco = [
            f"{prefix}/{route}"
            for prefix in prefixes
            for route in ("sst/program", "sst/cpu-program")
        ]
        vihaco.extend(
            f"instruction/const/sst/{route}"
            for route in ("cpu-operation", "cpu", "composite")
        )
        references = [
            f"{prefix}/{route}"
            for prefix in prefixes
            for route in ("rust/program", "python/program")
        ]
        return cls(tuple(vihaco), tuple(references))


@dataclass(frozen=True)
class Estimate:
    mean_ns: float
    lower_ns: float
    upper_ns: float
    samples: int
    uncertainty: str


Measurements = dict[str, Estimate]


@dataclass(frozen=True)
class Manifest:
    schema_version: int
    run_id: str
    reference_policy: str
    run_attempt: str
    profile: Profile
    head_sha: str
    base_sha: str
    suite_sha256: str
    workloads: dict[str, str]
    created_at: str
    environment: dict[str, str | int | None]
    dirty: bool
    machine_sha256: str | None = None
    base_machine_sha256: str | None = None
    base_machine_sha: str | None = None
    lock_sha256: str | None = None
    base_lock_sha256: str | None = None


@dataclass(frozen=True)
class ComparisonResult:
    manifest: Manifest
    base_status: BaseStatus
    base: Measurements
    candidate: Measurements
    references: Measurements
