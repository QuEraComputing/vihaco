# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from runner.audit import Scaling, native_assembly, scaling_cases, verify_workloads
from runner.contracts import discover
from runner.models import Estimate

WORKLOADS = Path(__file__).resolve().parents[2] / "workloads"


class AuditTest(unittest.TestCase):
    def test_flat_or_overlapping_timings_request_review(self) -> None:
        short = Estimate(10, 9, 11, 10, "test")
        flat = Scaling.compare(short, short)
        self.assertEqual(flat.mean_ratio, 1)
        self.assertIn("REVIEW", flat.assessment)
        growing = Scaling.compare(short, Estimate(100, 90, 110, 10, "test"))
        self.assertEqual(growing.mean_ratio, 10)
        self.assertIn("growth observed", growing.assessment)

    def test_all_workloads_have_controlled_scaling_cases(self) -> None:
        for workload in discover(WORKLOADS):
            with self.subTest(workload=workload.id):
                short, long = scaling_cases(workload)
                self.assertEqual(short.seed, long.seed)
                self.assertGreater(long.iterations, short.iterations)

    def test_different_seeds_are_not_a_scaling_comparison(self) -> None:
        workload = discover(WORKLOADS)[0]
        changed = replace(
            workload,
            cases=tuple(
                replace(case, seed=case.seed + 1) if case.id == "long" else case
                for case in workload.cases
            ),
        )
        with self.assertRaisesRegex(ValueError, "same-seed"):
            scaling_cases(changed)

    def test_stale_results_are_rejected(self) -> None:
        workloads = discover(WORKLOADS)
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            manifest = run / "manifest.json"
            manifest.write_text(
                json.dumps({"workloads": {item.id: item.digest for item in workloads}})
            )
            verify_workloads(run, workloads)
            manifest.write_text('{"workloads": {}}')
            with self.assertRaisesRegex(ValueError, "fingerprints differ"):
                verify_workloads(run, workloads)

    def test_assembly_includes_helpers_but_not_unrelated_functions(self) -> None:
        assembly = (
            "00001000 <vihaco_benchmark::workloads::workload_0::run::h123>:\n"
            "1000: bl 0x2000\n\n"
            "00002000 <vihaco_benchmark::workloads::workload_0::helper::h456>:\n"
            "2000: ret\n\n"
            "00003000 <unrelated>:\n"
            "3000: ret\n"
        )
        selected = native_assembly(assembly, 1)
        self.assertIn("helper", selected)
        self.assertNotIn("unrelated", selected)
        with self.assertRaisesRegex(ValueError, "symbols missing"):
            native_assembly(assembly, 2)
