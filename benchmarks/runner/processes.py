# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Subprocess lifecycle, log tailing, and progress outside measured code."""

import os
import signal
import subprocess
import time
from collections import deque
from collections.abc import Iterable, Mapping, Sequence
from functools import singledispatch
from pathlib import Path
from typing import TextIO

from .progress import Progress


class LogTail:
    """Consume appended output without losing partial lines between polls."""

    def __init__(self, reader: TextIO, progress: Progress) -> None:
        self.reader = reader
        self.progress = progress
        self.pending = ""
        self.recent: deque[str] = deque(maxlen=8)

    def update(self, *, finished: bool = False) -> None:
        self.pending += self.reader.read()
        lines = self.pending.split("\n")
        self.pending = lines.pop()
        if finished and self.pending:
            lines.append(self.pending)
            self.pending = ""
        for line in lines:
            self.recent.append(line)
            self.progress.observe(line)


def stop_process(process: subprocess.Popen[str]) -> None:
    # Workers inherit the process group, so timeouts also stop pyperf children.
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    process.wait()


@singledispatch
def failure_status(error: BaseException) -> str:
    return "failed"


@failure_status.register
def _timeout_status(error: subprocess.TimeoutExpired) -> str:
    return "timed out"


@failure_status.register
def _interrupt_status(error: KeyboardInterrupt) -> str:
    return "interrupted"


def wait_for_process(
    process: subprocess.Popen[str],
    command: Sequence[str],
    timeout: float,
    tail: LogTail,
) -> None:
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise subprocess.TimeoutExpired(command, timeout)
        try:
            process.wait(timeout=min(1, remaining))
        except subprocess.TimeoutExpired:
            tail.update()
            tail.progress.render()
        else:
            tail.update(finished=True)
            if process.returncode:
                raise subprocess.CalledProcessError(process.returncode, command)
            tail.progress.render("done")
            return


def execute(
    command: Sequence[str],
    cwd: Path | str,
    log: Path,
    env: Mapping[str, str] | None = None,
    timeout: float = 7200,
    *,
    label: str | None = None,
    benchmarks: Iterable[str] = (),
) -> None:
    progress = Progress(label or f"{log.parent.name}: {log.stem}", benchmarks)
    progress.render()
    with (
        log.open("w") as stream,
        log.open(errors="replace") as reader,
        subprocess.Popen(
            command,
            cwd=cwd,
            env=env,
            stdout=stream,
            stderr=subprocess.STDOUT,
            start_new_session=os.name == "posix",
            text=True,
        ) as process,
    ):
        tail = LogTail(reader, progress)
        try:
            wait_for_process(process, command, timeout, tail)
        except BaseException as error:
            # Cleanup includes interrupts; preserve the original failure.
            stop_process(process)
            tail.update(finished=True)
            progress.render(failure_status(error))
            print("Subprocess diagnostics retained locally; not echoed.", flush=True)
            raise


def output(command: Sequence[str], cwd: Path) -> str:
    """Capture short metadata commands that do not need a progress display."""
    return subprocess.check_output(
        command, cwd=cwd, text=True, stderr=subprocess.PIPE
    ).strip()
