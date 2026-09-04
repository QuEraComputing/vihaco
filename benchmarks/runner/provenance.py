# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Capture reproducibility metadata and serialize the existing artifact schema."""

import json
import os
import platform
import re
from collections.abc import Sequence
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path
from uuid import uuid4

from .models import ComparisonResult, Manifest, Profile, WorkloadSpec
from .processes import output
from .suite import fingerprint


def environment(repo: Path) -> dict[str, str | int | None]:
    # Publish only bounded, approved fields. Version banners, CPU descriptions,
    # OS build strings, and compiler flags can contain private paths or settings.
    return {
        "platform": approved(platform.system(), {"Linux", "Darwin", "Windows"}),
        "machine": approved(
            platform.machine(), {"arm64", "aarch64", "x86_64", "AMD64"}
        ),
        "python": version(platform.python_version()),
        "rust": version(output(["rustc", "--version"], repo).removeprefix("rustc ")),
        "cargo": version(output(["cargo", "--version"], repo).removeprefix("cargo ")),
        "build_flags_source": build_flags_source(),
        "cpu_count": os.cpu_count(),
    }


def approved(value: str, choices: set[str]) -> str:
    return value if value in choices else "other"


def version(value: str) -> str:
    match = re.match(r"^(\d+\.\d+\.\d+)(?:\s|$|-)", value)
    return match[1] if match else "unknown"


def build_flags_source() -> str:
    # Presence, including an empty value, determines Cargo's environment
    # precedence. This does not describe flags from Cargo config or rustc args.
    if "CARGO_ENCODED_RUSTFLAGS" in os.environ:
        return "CARGO_ENCODED_RUSTFLAGS"
    if "RUSTFLAGS" in os.environ:
        return "RUSTFLAGS"
    return "unset"


def run_identity() -> tuple[str, str]:
    """Never derive published identifiers from local paths or arbitrary text."""
    run_id = os.environ.get("GITHUB_RUN_ID")
    attempt = os.environ.get("GITHUB_RUN_ATTEMPT", "1")
    if not re.fullmatch(r"[1-9][0-9]{0,19}", attempt):
        raise ValueError("invalid run attempt")
    if run_id is None:
        if "GITHUB_RUN_ATTEMPT" in os.environ:
            raise ValueError("run attempt requires a run ID")
        return f"local-{uuid4().hex}", "1"
    if not re.fullmatch(r"[1-9][0-9]{0,19}", run_id):
        raise ValueError("invalid run ID")
    return run_id, attempt


def capture(
    suite: Path,
    destination: Path,
    profile: Profile,
    target: str | None,
    workloads: Sequence[WorkloadSpec],
) -> Manifest:
    repo = suite.parent
    run_id, run_attempt = run_identity()
    head = output(["git", "rev-parse", "HEAD"], repo)
    base = output(["git", "merge-base", "HEAD", target or "HEAD"], repo)
    return Manifest(
        schema_version=3,
        run_id=run_id,
        reference_policy="once per comparison; native Rust uses the candidate suite build",
        run_attempt=run_attempt,
        profile=profile,
        head_sha=head,
        base_sha=base,
        suite_sha256=fingerprint(suite),
        workloads={workload.id: workload.digest for workload in workloads},
        created_at=datetime.now(UTC).isoformat(),
        environment=environment(repo),
        dirty=bool(
            output(
                ["git", "status", "--porcelain", "--untracked-files=normal"],
                repo,
            )
        ),
    )


def write_json(path: Path, value: Manifest | ComparisonResult) -> None:
    path.write_text(json.dumps(asdict(value), indent=2) + "\n")
