# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

import contextlib
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from runner.processes import LogTail, execute
from runner.progress import Progress


class ProgressTest(unittest.TestCase):
    def test_counts_results_once_not_warmup_or_analysis(self) -> None:
        stream = io.StringIO()
        progress = Progress("Rust/SST", ["arithmetic/short/sst/program"], stream=stream)
        for stage in ("Warming up for 50 ms", "Collecting 10 samples", "Analyzing"):
            progress.observe(f"Benchmarking arithmetic/short/sst/program: {stage}")
            self.assertEqual(len(progress.completed), 0)
        progress.observe("\x1b[32m time: [1.0 us 1.1 us 1.2 us]\x1b[0m")
        progress.observe("Benchmarking arithmetic/short/sst/program: Analyzing")
        progress.observe("time: [1.0 us 1.1 us 1.2 us]")
        self.assertEqual(len(progress.completed), 1)
        progress.render("done")
        self.assertIn("[####################] 1/1", stream.getvalue())

    def test_python_summaries_and_unknown_output(self) -> None:
        progress = Progress(
            "Python", ["dispatch/long/python/program"], stream=io.StringIO()
        )
        progress.observe("...")
        progress.observe("unknown: 5 ns")
        self.assertFalse(progress.completed)
        progress.observe(
            "dispatch/long/python/program: Mean +- std dev: 9.13 us +- 0.03 us"
        )
        self.assertEqual(progress.completed, {"dispatch/long/python/program"})

    def test_nonterminal_heartbeat_is_throttled_and_has_no_control_codes(self) -> None:
        now = [0]
        stream = io.StringIO()
        progress = Progress("base: build", stream=stream, clock=lambda: now[0])
        progress.render()
        now[0] = 1
        progress.observe("Compiling vihaco")
        progress.render()
        self.assertEqual(len(stream.getvalue().splitlines()), 1)
        now[0] = 10
        progress.render()
        text = stream.getvalue()
        self.assertIn("0:10 process running", text)
        self.assertNotIn("Compiling vihaco", text)
        self.assertNotIn("\r", text)
        self.assertNotIn("\x1b", text)

    def test_terminal_updates_in_place_and_ends_with_newline(self) -> None:
        class Terminal(io.StringIO):
            def isatty(self) -> bool:
                return True

        stream = Terminal()
        progress = Progress("candidate: build", stream=stream)
        progress.render()
        progress.render("done")
        self.assertTrue(stream.getvalue().startswith("\r"))
        self.assertTrue(stream.getvalue().rstrip().endswith("done"))
        self.assertTrue(stream.getvalue().endswith("\n"))


class ExecutionTest(unittest.TestCase):
    def test_log_tail_preserves_partial_lines_until_completion(self) -> None:
        stream = io.StringIO("example/short/python")
        progress = Progress(
            "Python", ["example/short/python/program"], stream=io.StringIO()
        )
        tail = LogTail(stream, progress)
        tail.update()
        self.assertFalse(progress.completed)
        offset = stream.tell()
        stream.write("/program: 10 ns")
        stream.seek(offset)
        tail.update(finished=True)
        self.assertEqual(progress.completed, {"example/short/python/program"})
        self.assertEqual(list(tail.recent), ["example/short/python/program: 10 ns"])

    def test_preserves_full_log_and_tracks_actual_subprocess_results(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "python.log"
            capture = io.StringIO()
            with contextlib.redirect_stdout(capture):
                execute(
                    [
                        sys.executable,
                        "-c",
                        "print('example/short/python/program: 10 ns'); print('last line')",
                    ],
                    directory,
                    log,
                    benchmarks=["example/short/python/program"],
                )
            self.assertEqual(
                log.read_text(), "example/short/python/program: 10 ns\nlast line\n"
            )
            self.assertIn("1/1", capture.getvalue())
            self.assertIn("done", capture.getvalue())

    def test_failure_keeps_diagnostics_local_and_preserves_exit_status(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "failure.log"
            capture = io.StringIO()
            with (
                contextlib.redirect_stdout(capture),
                self.assertRaises(subprocess.CalledProcessError) as error,
            ):
                execute(
                    [
                        sys.executable,
                        "-c",
                        "print('validation failed'); raise SystemExit(7)",
                    ],
                    directory,
                    log,
                )
            self.assertEqual(error.exception.returncode, 7)
            self.assertNotIn(str(log), capture.getvalue())
            self.assertNotIn("validation failed", capture.getvalue())
            self.assertIn("validation failed", log.read_text())

    def test_timeout_terminates_silent_subprocess(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            capture = io.StringIO()
            with (
                contextlib.redirect_stdout(capture),
                self.assertRaises(subprocess.TimeoutExpired),
            ):
                execute(
                    [sys.executable, "-c", "import time; time.sleep(60)"],
                    directory,
                    Path(directory) / "timeout.log",
                    timeout=0.1,
                )
            self.assertIn("timed out", capture.getvalue())


if __name__ == "__main__":
    unittest.main()
