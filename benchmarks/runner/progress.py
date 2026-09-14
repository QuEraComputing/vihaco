# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Low-frequency progress display driven by the timing tools' existing output."""

import re
import shutil
import sys
import time
from collections.abc import Callable, Iterable
from typing import TextIO

ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


class Progress:
    def __init__(
        self,
        label: str,
        benchmarks: Iterable[str] = (),
        *,
        stream: TextIO | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.label = label
        self.expected = set(benchmarks)
        self.completed: set[str] = set()
        self.current = "waiting for output"
        self.active: str | None = None
        self.stream = stream if stream is not None else sys.stdout
        self.terminal = self.stream.isatty()
        self.clock = clock
        self.started = clock()
        self.last_render = self.started
        self.last_count = -1
        self.width = 0

    def observe(self, line: str) -> None:
        text = ANSI.sub("", line).strip()
        if not text:
            return
        if text.startswith("Benchmarking "):
            # Criterion emits a bare ID, then warmup/collection/analysis lines.
            name = text.removeprefix("Benchmarking ").split(":", 1)[0]
            if name in self.expected:
                self.active = name
                self.current = "measurement running"
        elif self.active and "time:" in text:
            self.completed.add(self.active)
            self.current = "measurement completed"
            self.active = None
        elif text.partition(":")[0] in self.expected:
            # pyperf's final per-benchmark summary follows its worker runs.
            name = text.partition(":")[0]
            self.completed.add(name)
            self.current = "measurement completed"
        elif not self.expected:
            self.current = "process running"

    def render(self, status: str | None = None) -> None:
        now = self.clock()
        count = len(self.completed)
        if (
            not self.terminal
            and status is None
            and count == self.last_count
            and now - self.last_render < 10
        ):
            return
        elapsed = int(now - self.started)
        duration = f"{elapsed // 60}:{elapsed % 60:02d}"
        bar = self._bar(elapsed, count)
        message = f"{self.label} {bar} {duration} {status or self.current}"
        if self.terminal:
            columns = max(20, shutil.get_terminal_size().columns - 1)
            message = message[:columns]
            self.stream.write("\r" + message.ljust(min(self.width, columns)))
            self.width = len(message)
            if status:
                self.stream.write("\n")
        else:
            self.stream.write(message + "\n")
        self.stream.flush()
        self.last_render = now
        self.last_count = count

    def _bar(self, elapsed: int, count: int) -> str:
        if self.expected:
            total = len(self.expected)
            filled = 20 * count // total
            return f"[{'#' * filled}{'-' * (20 - filled)}] {count}/{total}"
        symbol = "|/-\\"[elapsed % 4] if self.terminal else "running"
        return f"[{symbol}]"
