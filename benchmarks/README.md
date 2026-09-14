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

## CLI reference

Show the comparison command's options with:

```sh
uv run --directory benchmarks python -m runner --help
```

Run a comparison with either reduced smoke sampling or normal full sampling:

```sh
uv run --directory benchmarks python -m runner \
  --profile smoke \
  --base main \
  --output ../target/benchmark-runs/example
```

The options are:

| Option | Description |
|---|---|
| `--profile smoke\|full` | Reduced or normal timing samples. Defaults to `smoke`. |
| `--base REVISION` | Compare against the merge base of `HEAD` and `REVISION`; without it, use committed `HEAD`. |
| `--output DIRECTORY` | Required new directory for reports, measurements, logs, and exports. |
| `--base-machine REVISION` | Take only `benchmarks/machine` from another revision for the baseline. |
| `--base-machine-path DIRECTORY` | Use a compatible local baseline machine crate; mutually exclusive with `--base-machine`. |

Choose the baseline options based on what you are comparing:

| Situation | Options to use |
|---|---|
| Compare local changes with the committed `HEAD` | Omit `--base` and machine overrides. |
| Compare local changes with a branch or revision | Add `--base REVISION`. The library baseline is its merge base with `HEAD`. |
| The selected library baseline has a compatible machine adapter | Use `--base REVISION` only. |
| The selected library baseline predates the machine adapter | Use `--base REVISION --base-machine MACHINE_REVISION`. The library still comes from the merge base; only `benchmarks/machine` comes from `MACHINE_REVISION`. |
| The compatible machine exists only in a local checkout or edited directory | Use `--base REVISION --base-machine-path PATH`. |

For example, compare against `main` while supplying the machine from the tip
of `main`:

```sh
uv run --directory benchmarks python -m runner \
  --profile smoke \
  --base main \
  --base-machine main \
  --output ../target/benchmark-runs/comparison-smoke
```

Use only one of `--base-machine` and `--base-machine-path`. The supplied
machine must compile against the library revision selected by `--base`; the
runner does not automatically substitute the candidate machine when the
baseline adapter is missing or incompatible.

Successful runs place shareable files in `<output>/publish/`:
`report.md`, `results.json`, `manifest.json`, and `samples.json`. Raw timing
files and diagnostics remain outside that directory and may contain local paths
or environment details.

Audit a completed run's native scaling with:

```sh
uv run --directory benchmarks python -m runner.audit --help
uv run --directory benchmarks python -m runner.audit \
  ../target/benchmark-runs/example
```

To inspect native disassembly, pass the exact Criterion executable used for the
run. Use `--objdump llvm-objdump` when GNU `objdump` is unavailable:

```sh
uv run --directory benchmarks python -m runner.audit \
  ../target/benchmark-runs/example \
  --binary path/to/measured/criterion-executable \
  --objdump llvm-objdump
```

The audit verifies workload fingerprints, compares native `short` and `long`
cases, and selects relevant workload functions and helpers from the executable.
Its findings are advisory and require human review; it does not prove that an
algorithm's intended work survived optimization.

Run all local quality checks and the Rust/SST correctness validator with:

```sh
uv run --directory benchmarks python -m runner.checks
cargo run --locked --manifest-path benchmarks/Cargo.toml
```

The quality-check command writes detailed logs under `target/benchmark-checks/`.
The Rust command validates workload contracts, native references, both SST
execution routes, and isolated constant-instruction routes without collecting
timing samples.

These module commands are internal worker entry points used by the comparison
runner. They are useful for debugging but are not standalone comparison tools:

```sh
python -m runner.validate_python <workloads-directory>
python -m runner.python_bench [pyperf-options]
```

Run them from `benchmarks/` (or use `uv run --directory benchmarks`); direct
execution of the individual `.py` files is not supported.

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
```

See [Workloads](HOW_IT_WORKS.md#workloads) when adding a program and
[Code organization](HOW_IT_WORKS.md#code-organization) when changing the runner.

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
```
