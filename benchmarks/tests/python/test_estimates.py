# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""File-boundary and schema checks independent of the timing executables."""

import json
import tempfile
import unittest
from dataclasses import asdict
from pathlib import Path

from runner.estimates import add_estimate, collect
from runner.models import Estimate, Measurements


class EstimatesTest(unittest.TestCase):
    def test_criterion_output_keeps_existing_json_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            sample = directory / "criterion" / "workload" / "new"
            sample.mkdir(parents=True)
            files = {
                "benchmark.json": {"full_id": "example/short/sst/program"},
                "estimates.json": {
                    "mean": {
                        "point_estimate": 10,
                        "confidence_interval": {"lower_bound": 9, "upper_bound": 11},
                    }
                },
                "sample.json": {"times": [9, 11]},
            }
            for filename, data in files.items():
                (sample / filename).write_text(json.dumps(data))
            rows = collect(directory, ["example/short/sst/program"])
            self.assertEqual(
                asdict(rows["example/short/sst/program"]),
                {
                    "mean_ns": 10.0,
                    "lower_ns": 9.0,
                    "upper_ns": 11.0,
                    "samples": 2,
                    "uncertainty": "95% Criterion bootstrap mean interval",
                },
            )
            with self.assertRaisesRegex(ValueError, "extra="):
                collect(directory, [])

    def test_duplicate_and_nonfinite_estimates_fail(self) -> None:
        rows: Measurements = {}
        valid = Estimate(10, 9, 11, 2, "test")
        add_estimate(rows, "example", valid)
        with self.assertRaisesRegex(ValueError, "duplicate measurement"):
            add_estimate(rows, "example", valid)
        for value in (0.0, -1.0, float("nan"), float("inf")):
            with (
                self.subTest(value=value),
                self.assertRaisesRegex(ValueError, "invalid timing"),
            ):
                add_estimate({}, "example", Estimate(value, 9, 11, 2, "test"))


if __name__ == "__main__":
    unittest.main()
