# vihaco benchmarks

This suite measures vihaco CPU execution and composite routing, with equivalent
native Rust, Python, and SST workloads.

See [How the benchmark suite works](HOW_IT_WORKS.md) for workload contracts,
timing boundaries, report calculations, and implementation details.

## Run locally

Install Rust, uv, and a Python version supported by
[pyproject.toml](pyproject.toml). From the repository root:

```sh
uv sync --directory benchmarks --locked
uv run --directory benchmarks python -m runner --profile smoke --base main --output ../target/benchmark-runs/<name>
```

Use `--profile full` for normal sampling. Each output directory must be new.
The baseline is the merge base of HEAD and `--base`; omit `--base` to compare
local changes against committed HEAD.

The runner shows progress and writes `report.md`, structured results, and raw
logs to the output directory. The files in `publish/` are intended for sharing;
check raw diagnostics for private information before sharing those too.
See [Results](HOW_IT_WORKS.md#results) for the export policy and help reading
the report.

## PR benchmarks

PR updates trigger a smoke run. Correctness and pipeline failures fail the
check; performance signals are advisory.

Collaborators with write access can request a full run on a same-repository PR:

```text
@github-actions run benchmark
```

The PR author can explicitly publish a successful full comparison of the
current PR head:

```text
@github-actions commit benchmark <run-id>
```

Published reports live under `benchmarks/results/pr-N/run-ID/`. See
[CI and result publication](HOW_IT_WORKS.md#ci-and-result-publication) for
publication requirements, fork behavior, and permissions.

## Layout

```text
benchmarks/
  src/             Rust fixture, SST resolution, and workload discovery
  benches/         Criterion measurements
  workloads/       Equivalent Rust, Python, and SST program bundles
  runner/          Python comparison CLI, validation, and reporting
  tests/python/    Python support-code tests
  ci/              Trusted GitHub automation
    tests/         GitHub API and artifact tests
  results/         Reports published only on explicit author request
```

See [Workloads](HOW_IT_WORKS.md#workloads) when adding a program and
[Code organization](HOW_IT_WORKS.md#code-organization) when changing the runner
or CI support.

For native optimization checks, see
[Auditing comparable work](HOW_IT_WORKS.md#auditing-comparable-work).

## Development checks

After installing dependencies, run from the repository root:

```sh
uv run --directory benchmarks pyright --project ../pyrightconfig.json
uv run --directory benchmarks ruff check runner tests/python
uv run --directory benchmarks ruff format --check runner tests/python
cargo test --manifest-path benchmarks/Cargo.toml --all-targets
uv run --directory benchmarks python -m unittest discover -s tests/python -p 'test_*.py'
node --test benchmarks/ci/tests/ci.test.cjs
```
