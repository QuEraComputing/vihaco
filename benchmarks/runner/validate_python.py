# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Import and validate Python workloads inside a supervised child process."""

import sys
from pathlib import Path

from .contracts import discover

if __name__ == "__main__":
    discover(Path(sys.argv[1]))
