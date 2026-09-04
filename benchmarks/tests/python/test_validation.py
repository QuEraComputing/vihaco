# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Boundary dispatch must reject coercions and preserve exception inheritance."""

import subprocess
import unittest

from runner.contracts import parse_case
from runner.models import Workload, WorkloadCase
from runner.processes import failure_status
from runner.validation import require_integer, require_list, require_table, require_text


class ValidationTest(unittest.TestCase):
    def test_supported_input_types_pass_through(self) -> None:
        self.assertEqual(require_text("example"), "example")
        self.assertEqual(require_integer(42), 42)
        self.assertEqual(require_list([1, "two"]), [1, "two"])
        self.assertEqual(require_table({"id": 42}), {"id": 42})

    def test_integers_are_not_coerced_from_booleans_strings_or_floats(self) -> None:
        for value in (True, False, "1", 1.0, None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                require_integer(value)

    def test_other_input_shapes_are_rejected(self) -> None:
        for value in (1, True, None, [], {}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                require_text(value)
        for value in ({}, "cases", (), None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                require_list(value)
        for value in ([], "table", None, {1: "non-text key"}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                require_table(value)

    def test_case_fields_remain_required_and_typed(self) -> None:
        case = {"id": "short", "iterations": 1, "seed": 7, "expected": 7}
        self.assertEqual(parse_case(case, set()), WorkloadCase("short", 1, 7, 7))
        for changed in ({**case, "id": 1}, {**case, "extra": 1}, {"id": "short"}):
            with self.subTest(case=changed), self.assertRaises(ValueError):
                parse_case(changed, set())

    def test_boolean_result_cannot_match_integer_expectation(self) -> None:
        workload = Workload(
            "example",
            "algorithm",
            "constraints",
            (WorkloadCase("short", 1, 0, 1),),
            "digest",
            lambda iterations, seed: True,
        )
        with self.assertRaisesRegex(ValueError, "Python mismatch"):
            workload.validate()

    def test_exception_dispatch_handles_subclasses(self) -> None:
        class WorkerTimeout(subprocess.TimeoutExpired):
            pass

        class UserInterrupt(KeyboardInterrupt):
            pass

        self.assertEqual(failure_status(WorkerTimeout(["worker"], 1)), "timed out")
        self.assertEqual(failure_status(UserInterrupt()), "interrupted")
        self.assertEqual(failure_status(RuntimeError("failure")), "failed")


if __name__ == "__main__":
    unittest.main()
