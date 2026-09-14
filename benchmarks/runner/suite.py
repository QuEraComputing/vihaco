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


def fingerprint(root: Path, excluded: tuple[str, ...] = ()) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if relative.parts[0] in excluded or any(
            part in IGNORED_DIRECTORIES for part in relative.parts
        ):
            continue
        if path.is_symlink():
            raise ValueError(f"symlinks are not allowed in the suite: {relative}")
        if path.is_file():
            # Length-prefix both names and bytes so concatenation is unambiguous.
            for data in (relative.as_posix().encode(), path.read_bytes()):
                digest.update(len(data).to_bytes(8, "big"))
                digest.update(data)
    return digest.hexdigest()


def harness_fingerprint(suite: Path) -> str:
    """The shared driver excludes checkout-owned machines and dependency locks."""
    return fingerprint(suite, ("machine", "Cargo.lock"))


def file_digest(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def archive_revision(repo: Path, revision: str, destination: Path) -> None:
    archive = destination.parent / f"{destination.name}.tar"
    destination.mkdir()
    subprocess.run(
        ["git", "archive", "--format=tar", "--output", str(archive), revision],
        cwd=repo,
        check=True,
        capture_output=True,
    )
    with tarfile.open(archive) as source:
        source.extractall(destination, filter="data")


@contextmanager
def base_checkout(
    suite: Path,
    revision: str,
    machine_revision: str | None = None,
    machine_path: Path | None = None,
) -> Iterator[Path]:
    """Overlay one shared harness while preserving the selected machine and lock."""
    if machine_revision is not None and machine_path is not None:
        raise ValueError("select only one base machine override")
    digest = harness_fingerprint(suite)
    with tempfile.TemporaryDirectory(prefix="vihaco-benchmark-") as temporary:
        root = Path(temporary)
        checkout = root / "base"
        archive_revision(suite.parent, revision, checkout)
        base_suite = checkout / "benchmarks"
        if machine_revision is not None:
            override = root / "override"
            archive_revision(suite.parent, machine_revision, override)
            machine_path = override / "benchmarks/machine"
        if machine_path is not None:
            if not (machine_path / "Cargo.toml").is_file():
                raise ValueError("base machine override must contain Cargo.toml")
            if (base_suite / "machine").exists():
                shutil.rmtree(base_suite / "machine")
            shutil.copytree(
                machine_path,
                base_suite / "machine",
                ignore=shutil.ignore_patterns(*IGNORED_DIRECTORIES),
            )
        base_suite.mkdir(exist_ok=True)
        for path in base_suite.iterdir():
            if path.name in {"machine", "Cargo.lock"}:
                continue
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path)
            else:
                path.unlink()
        shutil.copytree(
            suite,
            base_suite,
            dirs_exist_ok=True,
            ignore=lambda directory, names: [
                name
                for name in names
                if name in IGNORED_DIRECTORIES
                or (Path(directory) == suite and name in {"machine", "Cargo.lock"})
            ],
        )
        if harness_fingerprint(base_suite) != digest:
            raise ValueError("harness changed while preparing the comparison")
        yield base_suite
