# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Suite identity and temporary merge-base checkouts."""

import hashlib
import shutil
import subprocess
import tarfile
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

IGNORED_DIRECTORIES = {"target", ".venv", ".ruff_cache", "__pycache__", "results"}


def fingerprint(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if any(part in IGNORED_DIRECTORIES for part in relative.parts):
            continue
        if path.is_symlink():
            raise ValueError(f"symlinks are not allowed in the suite: {relative}")
        if path.is_file():
            # Length-prefix both names and bytes so concatenation is unambiguous.
            for data in (relative.as_posix().encode(), path.read_bytes()):
                digest.update(len(data).to_bytes(8, "big"))
                digest.update(data)
    return digest.hexdigest()


@contextmanager
def base_checkout(suite: Path, revision: str, digest: str) -> Iterator[Path]:
    """Use the current suite against archived base code, leaving git untouched."""
    with tempfile.TemporaryDirectory(prefix="vihaco-benchmark-") as temporary:
        checkout = Path(temporary) / "base"
        checkout.mkdir()
        archive = Path(temporary) / "base.tar"
        subprocess.run(
            ["git", "archive", "--format=tar", "--output", str(archive), revision],
            cwd=suite.parent,
            check=True,
            capture_output=True,
        )
        with tarfile.open(archive) as source:
            source.extractall(checkout, filter="data")
        base_suite = checkout / "benchmarks"
        # This path belongs only to our disposable checkout.
        if base_suite.exists():
            shutil.rmtree(base_suite)
        shutil.copytree(
            suite,
            base_suite,
            ignore=shutil.ignore_patterns(*IGNORED_DIRECTORIES),
        )
        if fingerprint(base_suite) != digest:
            raise ValueError("suite changed while preparing the comparison")
        yield base_suite
