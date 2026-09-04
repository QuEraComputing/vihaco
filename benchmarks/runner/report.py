# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Render typed results as Markdown; no timing or filesystem access."""

from .models import ComparisonResult, Estimate, Manifest


def comparison(base: Estimate, candidate: Estimate) -> tuple[float, str]:
    delta = 100 * (candidate.mean_ns / base.mean_ns - 1)
    if candidate.lower_ns > base.upper_ns:
        signal = "possible slowdown"
    elif candidate.upper_ns < base.lower_ns:
        signal = "possible improvement"
    else:
        signal = "inconclusive"
    return delta, signal


def interval(estimate: Estimate) -> str:
    return f"{estimate.mean_ns:.2f} ({estimate.lower_ns:.2f}–{estimate.upper_ns:.2f})"


def header(manifest: Manifest) -> list[str]:
    return [
        "# vihaco benchmark results",
        "",
        f"Profile: **{manifest.profile}** · Run: `{manifest.run_id}`",
        "",
        f"Base: `{manifest.base_sha}` · Candidate: `{manifest.head_sha}`",
        f"Suite SHA-256: `{manifest.suite_sha256}`",
        "",
        "Times are nanoseconds per invocation. Negative change means faster.",
        "Intervals describe sampling uncertainty, not hardware drift. Signals are advisory;",
        "overlapping intervals are inconclusive. No multiple-comparison correction is applied.",
        "",
    ]


def revision_table(result: ComparisonResult) -> list[str]:
    lines: list[str] = []
    if result.base_status != "available":
        lines.extend(["Base comparison unavailable: see the base build log.", ""])
    lines.extend(
        [
            "| Workload / case / implementation / level | Base ns | Candidate ns (95% interval) | Change | Signal |",
            "|---|---:|---:|---:|---|",
        ]
    )
    for name, row in sorted(result.candidate.items()):
        baseline = result.base.get(name)
        base_text, change, signal = "—", "—", "unavailable"
        if baseline is not None:
            delta, signal = comparison(baseline, row)
            base_text, change = f"{baseline.mean_ns:.2f}", f"{delta:+.1f}%"
        lines.append(
            f"| {name} | {base_text} | {interval(row)} | {change} | {signal} |"
        )
    return lines


def reference_table(result: ComparisonResult) -> list[str]:
    lines = [
        "",
        "## Shared references",
        "",
        "Native Rust and Python are measured once per comparison, not per revision.",
        "Native Rust uses the candidate suite build. These are context, not regression comparisons.",
        "",
        "| Workload / case / implementation / level | Time ns (95% interval) |",
        "|---|---:|",
    ]
    lines.extend(
        f"| {name} | {interval(row)} |"
        for name, row in sorted(result.references.items())
    )
    return lines


def language_table(result: ComparisonResult) -> list[str]:
    lines = [
        "",
        "## Cross-language execution",
        "",
        "Ratios are descriptive point estimates, relative to the same shared native Rust measurement.",
        "",
        "| Workload / case | Base SST / Rust | Candidate SST / Rust | Python / Rust |",
        "|---|---:|---:|---:|",
    ]
    for name, native in sorted(result.references.items()):
        if not name.endswith("/rust/program"):
            continue
        prefix = name.removesuffix("/rust/program")
        vm = result.candidate[prefix + "/sst/program"].mean_ns
        python = result.references[prefix + "/python/program"].mean_ns
        baseline = result.base.get(prefix + "/sst/program")
        base_ratio = f"{baseline.mean_ns / native.mean_ns:.2f}×" if baseline else "—"
        lines.append(
            f"| {prefix} | {base_ratio} | {vm / native.mean_ns:.2f}× "
            f"| {python / native.mean_ns:.2f}× |"
        )
    return lines


def render(result: ComparisonResult) -> str:
    lines = [
        *header(result.manifest),
        *revision_table(result),
        *reference_table(result),
        *language_table(result),
    ]
    return "\n".join(lines) + "\n"
