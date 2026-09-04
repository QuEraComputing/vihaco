# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Check bundle expectations using calculations separate from the timed code."""

import unittest
from pathlib import Path

from runner.contracts import discover

WORKLOADS = Path(__file__).resolve().parents[2] / "workloads"


def fibonacci(n: int) -> int:
    # An iterative recurrence checks the recursive implementation's outputs.
    current, following = 0, 1
    for _ in range(n):
        current, following = following, current + following
    return current


def rotate_bits(value: int, left: int) -> int:
    # String rearrangement avoids repeating the measured shift/mask formulas.
    bits = f"{value:064b}"
    return int(bits[left:] + bits[:left], 2)


def bitwise(iterations: int, seed: int) -> int:
    value, other = seed, 305419896
    for _ in range(iterations):
        rotated = rotate_bits(value ^ other, 51)
        shifted = (rotated * 128) % (2**64)
        value = (shifted & other) ^ seed
        other = rotate_bits(other, 3)
    return value % 65536


def large_frame(iterations: int, seed: int) -> int:
    # Model slot updates as successive ring rotations, not indexed mutation.
    ring = tuple(seed + index for index in range(10))
    for _ in range(iterations * 10):
        updated = (17 * ring[0] + ring[-1]) % 65521
        ring = (*ring[1:], updated)
    return sum(ring)


def dispatch(iterations: int, seed: int) -> int:
    value = seed
    for index in range(iterations):
        # Rotate a 16-character bit string before mixing in the loop index.
        bits = f"{value:016b}"
        value = (int(bits[1:] + bits[:1], 2) ^ index) % 65536
    return value


class WorkloadOraclesTest(unittest.TestCase):
    def test_bundle_expectations_have_independent_oracles(self) -> None:
        workloads = {workload.id: workload for workload in discover(WORKLOADS)}
        for name in (
            "recursive_fibonacci",
            "large_frame",
            "bitwise",
            "arith_pow_squaring",
            "dispatch",
        ):
            for case in workloads[name].cases:
                with self.subTest(workload=name, case=case.id):
                    match name:
                        case "recursive_fibonacci":
                            expected = fibonacci(case.iterations) + case.seed
                        case "large_frame":
                            expected = large_frame(case.iterations, case.seed)
                        case "dispatch":
                            expected = dispatch(case.iterations, case.seed)
                        case "bitwise":
                            expected = bitwise(case.iterations, case.seed)
                        case "arith_pow_squaring":
                            expected = pow(case.seed, case.iterations, 65521)
                        case _:
                            self.fail(f"missing oracle: {name}")
                    self.assertEqual(case.expected, expected)
