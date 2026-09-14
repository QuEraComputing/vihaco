# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Validate workload contracts and load their Python reference functions."""

import importlib.util
import re
import tomllib
from pathlib import Path
from typing import cast

from .models import Workload, WorkloadCase, WorkloadFunction, WorkloadSpec
from .suite import fingerprint
from .validation import require_integer, require_list, require_table, require_text

IDENTIFIER = re.compile(r"[a-z0-9_-]+")
CONTRACT_FIELDS = {"id", "algorithm", "numeric_constraints", "limits", "cases"}
CASE_FIELDS = {"id", "iterations", "seed", "expected"}
IMPLEMENTATIONS = ("native.rs", "python.py")


def unique_id(value: str, seen: set[str], kind: str) -> str:
    if not IDENTIFIER.fullmatch(value) or value in seen:
        raise ValueError(f"invalid or duplicate {kind} id: {value}")
    seen.add(value)
    return value


def parse_case(raw: object, seen: set[str]) -> WorkloadCase:
    fields = require_table(raw)
    if set(fields) != CASE_FIELDS:
        raise ValueError("invalid case fields")
    name = unique_id(require_text(fields["id"]), seen, "case")
    iterations = require_integer(fields["iterations"])
    seed = require_integer(fields["seed"])
    expected = require_integer(fields["expected"])
    if not (
        0 <= iterations <= 1_000_000 and 0 <= seed < 65_536 and 0 <= expected < 2**64
    ):
        raise ValueError("case outside numeric constraints")
    return WorkloadCase(name, iterations, seed, expected)


def load_function(path: Path, name: str) -> WorkloadFunction:
    spec = importlib.util.spec_from_file_location(f"workload_{name}", path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load Python implementation: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    function = getattr(module, "run", None)
    if not callable(function):
        raise TypeError(f"missing callable run(iterations, seed): {path}")
    # Python plugins cannot prove their signature statically. Validation invokes
    # every declared case and checks the result before the function is measured.
    return cast(WorkloadFunction, function)


def load_spec(path: Path, seen: set[str]) -> WorkloadSpec:
    """Read bundle metadata without importing or executing its implementation."""
    raw = tomllib.loads((path / "workload.toml").read_text())
    if set(raw) != CONTRACT_FIELDS:
        raise ValueError(f"invalid contract fields: {path}")
    name = unique_id(require_text(raw["id"]), seen, "workload")
    algorithm = require_text(raw["algorithm"])
    constraints = require_text(raw["numeric_constraints"])
    if not algorithm or not constraints:
        raise ValueError(f"missing contract: {name}")
    raw_cases = require_list(raw["cases"])
    if len(raw_cases) < 2:
        raise ValueError("multiple validation cases required")
    case_ids: set[str] = set()
    cases = tuple(parse_case(case, case_ids) for case in raw_cases)
    limits = require_table(raw["limits"])
    if set(limits) != {"max_iterations", "max_seed"}:
        raise ValueError("invalid workload limits")
    max_iterations = require_integer(limits["max_iterations"])
    max_seed = require_integer(limits["max_seed"])
    if not (0 <= max_iterations <= 1_000_000 and 0 <= max_seed < 65_536):
        raise ValueError("invalid workload limits")
    if any(case.iterations > max_iterations or case.seed > max_seed for case in cases):
        raise ValueError("case exceeds workload limits")
    for filename in IMPLEMENTATIONS:
        if not (path / filename).is_file():
            raise ValueError(f"missing {filename} in {path}")
    return WorkloadSpec(name, algorithm, constraints, cases, fingerprint(path))


def load_workload(path: Path, seen: set[str]) -> Workload:
    spec = load_spec(path, seen)
    workload = Workload(
        spec.id,
        spec.algorithm,
        spec.numeric_constraints,
        spec.cases,
        spec.digest,
        load_function(path / "python.py", spec.id),
    )
    workload.validate()
    return workload


def discover_specs(root: Path) -> list[WorkloadSpec]:
    seen: set[str] = set()
    workloads = [
        load_spec(path, seen) for path in sorted(root.iterdir()) if path.is_dir()
    ]
    if not workloads:
        raise ValueError("no workloads")
    return workloads


def discover(root: Path) -> list[Workload]:
    seen: set[str] = set()
    workloads = [
        load_workload(path, seen) for path in sorted(root.iterdir()) if path.is_dir()
    ]
    if not workloads:
        raise ValueError("no workloads")
    return workloads
