# vihaco benchmarks

This suite measures vihaco CPU execution and composite routing, with equivalent
native Rust, Python, and SST workloads.

See [How the benchmark suite works](HOW_IT_WORKS.md) for workload contracts,
timing boundaries, report calculations, and implementation details.

## Run locally

Install Rust, uv, and a Python version supported by
[pyproject.toml](pyproject.toml). From the candidate checkout's repository root:

```sh
uv sync --directory benchmarks --locked
uv run --directory benchmarks python -m runner --profile smoke --base main --output ../target/benchmark-runs/<name>
```

Use `--profile full` for normal sampling. Each output directory must be new.
The baseline is the merge base of HEAD and `--base`; omit `--base` to compare
local changes against committed HEAD. The merge base may be older than the
named branch's tip.

To measure an experiment, use the runner in that experiment's checkout.
`--base` only chooses the baseline. The candidate measurements include local
edits to the library and machine.

Both revisions use the current harness and workload contracts. Each checkout
supplies `benchmarks/machine`, which implements the shared traits in
`benchmarks/api` using its own vihaco API and SST syntax. Breaking library changes
therefore require updating the machine adapter alongside the library.

For an older baseline without an adapter, pass `--base-machine <revision>` or
`--base-machine-path <machine-crate-directory>` to supply one compatible with
that baseline. These options select only the machine; `--base` still selects
the library merge base. Without an adapter, the baseline is unavailable.

If `main` has a compatible machine but the merge base predates it, run this
from the candidate checkout:

```sh
uv run --directory benchmarks python -m runner \
  --profile smoke --base main --base-machine main \
  --output ../target/benchmark-runs/comparison-smoke
```

The supplied machine must work with the baseline's API. When copying a newer
harness into an older experiment checkout, keep the experiment's own machine.
Check that it builds and passes validation before measuring:

```sh
cargo test --locked --manifest-path benchmarks/Cargo.toml --workspace --all-targets
```

To run from the main repository using an existing sibling worktree, pass
`--directory ../<worktree>/benchmarks`. The runner resolves relative `--output`
and `--base-machine-path` paths from that benchmark directory. Use an absolute
machine path if you're unsure.

The runner shows progress and writes `report.md`, structured results, and raw
logs to the output directory. The files in `publish/` are intended for sharing;
check raw diagnostics for private information before sharing those too.
See [Results](HOW_IT_WORKS.md#results) for the export policy and help reading
the report.

If validation fails, read `failure.log` and `candidate/validation.log` in the
output directory. If the baseline is unavailable, check `base/resolve.log` and
`base/build.log` if they exist. After fixing the problem, rerun with a new output
directory.

## PR benchmarks

PR updates trigger a smoke run. Correctness and pipeline failures fail the
check; performance signals are advisory.
CI uses the machine from the PR's merge base. If that revision has no machine,
the report shows only candidate and reference timings. Machine overrides are
available through the local CLI; the bot commands don't accept them. A CI
comparison needs a compatible machine at the merge base.

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
  src/             Shared workload discovery and validation
  api/             Stable traits, independent of vihaco
  machine/         Checkout-specific example machine, resolver, and SST programs
  benches/         Criterion measurements
  workloads/       Shared contracts and Rust/Python references
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
cargo fmt --manifest-path benchmarks/Cargo.toml --all -- --check
cargo clippy --manifest-path benchmarks/Cargo.toml --workspace --all-targets -- -D warnings
uv run --directory benchmarks pyright --project ../pyrightconfig.json
uv run --directory benchmarks ruff check runner tests/python
uv run --directory benchmarks ruff format --check runner tests/python
cargo test --manifest-path benchmarks/Cargo.toml --workspace --all-targets
uv run --directory benchmarks python -m unittest discover -s tests/python -p 'test_*.py'
node --test benchmarks/ci/tests/ci.test.cjs
```
