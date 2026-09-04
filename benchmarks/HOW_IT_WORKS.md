# How the benchmark suite works

The benchmark suite compares vihaco execution performance across two library
revisions. It runs the same programs against both revisions and measures
equivalent native Rust and Python implementations for reference. The suite has
its own unpublished Cargo workspace, a Python runner, and GitHub automation.
The separate workspace keeps benchmark dependencies out of the library and
allows the same suite to build against different vihaco revisions.

See the [README](README.md) for installation and commands.
The [design](../design/benchmarking.md) records the suite's scope and rationale.

## Workloads

Each directory in [workloads/](workloads/) contains:

- `workload.toml`: a stable ID, algorithm, numeric constraints, input limits, and cases
  with independently specified expected results.
- `native.rs`: `run(iterations: u64, seed: u64) -> u64`.
- `python.py`: `run(iterations, seed)`.
- `program.sst`: a complete `sst v1` container with a root section and text containing
  `@main(iterations: u64, seed: u64) -> u64`.

The implementations follow the algorithm in the manifest and receive the same
inputs for each case. When adding a workload, review the implementations for
equivalent work as well as checking their results.

Each manifest defines its own cases, including iteration counts, seeds, and
expected results. Cases can exercise boundary conditions, fixed invocation
overhead, or sustained execution. The manifest explains how the workload uses
the `iterations` input: it can be a loop count, an exponent, or a recursive
problem size. The timing tools repeat the entire invocation to collect samples.

The `[limits]` table sets `max_iterations` and `max_seed`. Both contract loaders
check these limits before executing any cases. Choose limits based on the
algorithm's cost and arithmetic bounds, not just the size of its input type.

[build.rs](build.rs) discovers native implementations and generates a registry
in sorted bundle order. [Rust discovery](src/workloads.rs) loads contracts and
SST programs; [Python discovery](runner/contracts.py) reads contracts separately
from loading Python functions. The runner imports and validates Python workloads
in a supervised child process. Missing implementations, duplicate IDs, invalid inputs, and wrong
results fail validation before timing.

## Comparing revisions

From the repository root:

```sh
uv run --directory benchmarks python -m runner --profile smoke --base main --output ../target/benchmark-runs/example
```

The output directory must be new. The candidate is the current working tree;
the baseline revision is `git merge-base HEAD main` in this example. Without
`--base`, the baseline is committed `HEAD`, which is useful for measuring local
edits. The manifest records `HEAD` and flags any uncommitted changes. Results
from a dirty working tree include those changes, so the recorded SHA is
insufficient to reproduce them. The bot only publishes results from clean
working trees.

[run.py](runner/run.py) orchestrates this sequence:

```text
Discover and validate Python workloads; capture provenance
  → Validate and build the candidate Rust suite
  → Archive the merge base into a temporary checkout
  → Copy the current suite into that checkout; build and validate it
  → Measure base SST routes
  → Measure candidate SST routes
  → Measure shared native Rust references
  → Measure shared Python references
  → Check suite identity and write results/report
```

[suite.py](runner/suite.py) copies the current suite into the temporary
checkout's `benchmarks/` directory. Its path dependencies then compile against
the older vihaco crates. This keeps the benchmark code consistent across the
comparison and leaves the user's checkout untouched.

Suite and workload SHA-256 fingerprints include relative file names and their
contents, excluding build output, environments, caches, and published results.
The runner checks the copied suite against the recorded fingerprint and checks
the original again after measurement. Editing the suite during a run causes
that check to fail.

After both builds finish, the runner measures the base and candidate
sequentially on the same machine. It then measures the native Rust and Python
references once each, using the candidate build for native Rust. The report
uses these shared references for both revisions' language comparisons.

[SuiteRunner](runner/measurement.py) marks the baseline as `unavailable` if it
cannot compile the suite, then continues with the candidate and references.
Validation failures, timeouts, and measurement failures stop the run.

## Loading and executing SST

[Program::parse](src/machine/program.rs) uses vihaco's normal loading pipeline:
`SstFile` → `ParsedModule::parse_section` → `Resolve` → `ProgramImage`.

The [resolver](src/resolve.rs) first assigns function indices and addresses,
then resolves function-local labels, signatures, constants, and instructions.
This allows forward calls and function references. The loader and resolver
define the SST features supported by the fixture.

[Fixture::for_program](src/machine.rs) prepares a fresh CPU frame at the resolved
`@main` entry address, supplies the case inputs, and initializes scratch locals.
Preparation reserves working stack storage outside timing.

During execution, the fixture fetches an instruction, supplies any required
CPU message, calls `execute_generated`, extracts its `StepOutcome`, and advances
or redirects the program counter. A halt must leave exactly one result above
the main function's locals; a return must expose exactly one return value.

## Measurements

Program measurement IDs have the form
`<workload>/<case>/<implementation>/<route>`. A program measurement runs a
complete invocation of the native Rust, Python, or SST implementation.
Isolated instruction measurements exercise an operation or dispatch path
without the full program loop.

The [Criterion driver](benches/workloads.rs) registers program measurements,
and its [support module](benches/support/mod.rs) registers isolated instruction
measurements. These registrations define the available execution routes.
The runner derives the expected measurement IDs from the discovered workloads
and checks the collected output against them.

SST execution uses the fixture's CPU dispatcher and the instruction types and
loading support generated by the composite macro. Compile-time parameters
select the execution route.

The [Criterion driver](benches/workloads.rs) loads and resolves programs before
timing. It uses `iter_batched_ref` to keep frame preparation and fixture
destruction outside the timed closure. The closure measures execution and
numeric result access, including any allocations required during execution.
The isolated instruction measurements also read the resulting stack value.

Native Rust uses black-box barriers around inputs and results while allowing
the compiler to optimize the workload's body. Each workload documents any
additional barriers in its own README. The
[pyperf entry point](runner/python_bench.py) times `run(iterations, seed)` with
`timeit`, avoiding an additional wrapper function in the timed expression.
Python worker startup and workload validation happen outside that expression.

The number of measurements depends on the discovered workloads, their cases,
and the registered execution routes. Criterion and pyperf execute each
measurement repeatedly to collect samples.

## Auditing comparable work

Correct results alone do not establish that the implementations perform
comparable work. A compiler can replace a loop with a formula while leaving
its answer unchanged. Review native assembly when changing a workload or
upgrading the compiler, and check timing growth across input sizes.

Keep each workload's audit notes in its README under [workloads/](workloads/).
Describe the work that must survive optimization, the transformations that are
acceptable, and how execution cost should grow with input size. Document any
internal black-box barriers and the overhead they add to the native reference.
Input/output black boxes alone do not preserve intermediate operations.

The comparisons cover equivalent algorithms without requiring identical machine
instructions or memory traffic. Use the standalone instruction measurements to
isolate individual VM operations.

After completing a comparison, run this from the repository root:

```sh
uv run --directory benchmarks python -m runner.audit ../target/benchmark-runs/example
```

The audit compares native `short` and `long` cases with the same seed and
rejects results whose workload fingerprints no longer match the checkout.
Its ratio range divides the stored interval endpoints; it is not a confidence
interval for the ratio. A flat or overlapping result requests review, while
observed growth still requires assembly inspection. Interpret timing growth
using the workload's audit notes; an input-size ratio is not necessarily the
expected timing ratio.

To include assembly, pass `--binary` with the exact Criterion executable used
for that run. The executable path appears in the build log. GNU or LLVM
`objdump` must be installed; use `--objdump llvm-objdump` to select it.
The audit prints the generated native-module mapping and extracts the workload
functions and helpers from the demangled disassembly. It does not execute the
binary or automatically prove that its control flow is correct. Confirm the
binary matches the run; the workload fingerprint check does not authenticate
an independently supplied executable.

Follow calls into helpers, locate the loop back edges and state updates, and
check for constant returns or closed-form replacements. Missing symbols require
manual investigation, not a passing audit. Timing signals remain advisory;
recheck questionable results with full sampling. Workload or barrier changes
also change the workload fingerprint, so their timings should not be treated
as a continuation of the old workload's history.

## Sampling and progress

`full` uses the timing tools' default sampling settings. `smoke` keeps the same
programs and inputs but reduces sampling. The
[Criterion configuration](benches/support/mod.rs) and
[Python measurement configuration](runner/measurement.py) define the sample
counts, warmup settings, and measurement durations.

Total run time also includes compilation, calibration, validation, and worker
startup. Workflow job timeouts and subprocess timeouts limit how long a run
can continue; their values are defined in the workflows and runner.

[processes.py](runner/processes.py) captures full child-process logs and polls
them for new output. [progress.py](runner/progress.py) updates the display as
Criterion and pyperf report completed measurements. During builds, it displays
elapsed time and whether the process is running. Terminal output updates in
place; CI and redirected output receive periodic plain-text updates. The parent
process handles progress reporting outside the timed benchmark code.
If a step fails, the runner reports which phase failed and keeps the subprocess
output, command arguments, and traceback in local logs. On POSIX systems,
timeout cleanup also stops child workers.

## Results

The output directory contains `manifest.json`, `results.json`, and `report.md`,
plus `base/`, `candidate/`, and `references/` directories with logs and raw
timing-tool output. Structured results record their schema version and keep
shared references separate from revision measurements.

The runner puts files intended for sharing in `publish/`:
`manifest.json`, `results.json`, `report.md`, and `samples.json`.
The sample file contains Criterion iteration counts and elapsed times, plus
pyperf values grouped by measured process. It leaves out the timing tools'
metadata, commands, calibration diagnostics, and raw logs. After a successful
run, CI uploads only these named files. The rest of the run directory, including
failure diagnostics, stays on the machine that ran the benchmarks.

The published environment fields are OS family, architecture, numeric
Python/Rust/Cargo versions, logical CPU count, and the environment variable
that supplied build flags. The runner uses `other` for unsupported platform
names and `unknown` for versions it cannot recognize. It omits verbose version
banners, CPU descriptions, OS build strings, and the flags themselves, including
fingerprints of their values.

`CARGO_ENCODED_RUSTFLAGS` takes precedence over `RUSTFLAGS`, even when its
value is empty. If neither variable is present, the runner records `unset`.
This field cannot tell you which compiler settings came from Cargo configuration
or command arguments.

Local logs and original timing-tool files may contain private paths or settings.
Check their contents before sharing them. The export rules help prevent
accidental disclosure, but cannot contain a malicious workload. Untrusted code
still needs an environment without secrets, restricted CI permissions, and a
checkout without credentials.

[estimates.py](runner/estimates.py) reads Criterion's mean estimates and
bootstrap intervals. For Python, it calculates each measured process's mean,
then bootstraps those process means with a fixed random seed. Calibration-only
runs are excluded. The estimation code defines the resampling settings and
minimum number of measured processes.
The reported sample count is the number of Criterion samples for Rust/SST and
the number of measured processes for Python. Collection fails if measurements
are missing, unexpected, duplicated, nonfinite, or nonpositive.

[report.py](runner/report.py) calculates revision changes as:

```text
change (%) = 100 × (candidate mean / base mean − 1)
```

A negative change indicates that the candidate ran faster. If the intervals
do not overlap, the report labels the change as a "possible improvement" or
"possible slowdown"; otherwise it labels it "inconclusive". These labels help
identify changes worth investigating and do not affect whether CI passes.
The calculation does not correct for multiple comparisons. Smoke runs provide
limited evidence because they use fewer samples.

The language-comparison table divides the mean time for each SST revision and
Python by the shared native Rust mean for that case. The report does not
calculate confidence intervals for these ratios. Their size depends on the
algorithm, input size, and compiler optimization opportunities.

The manifest records revisions, suite/workload fingerprints, toolchain and
environment details, and dirty state. Local runs use an opaque identifier that
does not include the output directory's name. GitHub run IDs and attempts must
be bounded positive decimal strings. Sampling intervals do not capture all
hardware drift or shared-runner noise. Use full runs and repeated evidence
before drawing conclusions about small performance changes.

## CI and result publication

The [smoke workflow](../.github/workflows/benchmark-smoke.yml) runs on PR updates.
Correctness and infrastructure failures fail the check; performance signals
remain advisory. Collaborators with write access can request a full run on a
same-repository PR:

```text
@github-actions run benchmark
```

The [command workflow](../.github/workflows/benchmark-command.yml) records the
authorized request and measured SHAs. Measurement jobs run PR code with
read-only repository permissions and no checkout credentials. A separate
[comment workflow](../.github/workflows/benchmark-comment.yml) uses trusted
default-branch code to post the report and mark results stale if the PR advances.
Before posting, it matches the report's run ID, attempt, profile, and head SHA
against the triggering run and its PR association or authorized request. Command
results must also match the requested base SHA. Mismatched metadata is rejected;
results from a genuine older revision can still be posted with a stale warning.

To commit the results of a completed run, the PR author comments:

```text
@github-actions commit benchmark <run-id>
```

[Publication checks](ci/policy.cjs) verify that the selected run completed a full
comparison on a clean working tree, has an available baseline, and matches the
authorized request, run attempt, and current PR head. The artifacts must still
be available. [Publication](ci/publication.cjs) commits `report.md` and
`results.json` under `results/pr-N/run-ID/` with a non-forced branch update.
If the PR advances during publication, the update fails. The workflows'
artifact retention settings determine how long CI keeps numeric samples.
Before posting a comment or committing results, trusted code from the default
branch checks the JSON against the allowed fields and values, then generates
the Markdown report. It does not copy report text from the downloaded artifact.
Results using the older metadata schema need a fresh run before publication.
The same checks apply when the PR author requests the commit.

Publication honors branch protection. Commits made with `GITHUB_TOKEN` do not
trigger ordinary push CI, so required checks on the report commit may need a
maintainer rerun.

Fork PRs receive smoke runs but cannot invoke bot commands. Comments on fork
runs depend on GitHub providing an associated PR; workflow summaries and
artifacts remain available regardless. The local runner
only writes its requested output directory and temporary/build files; it never
posts comments, commits, or pushes.

## Execution boundaries

Python imports and validation have a subprocess deadline, as do builds and
measurements. SST loading also limits source size, local slots per function, and
individual heap allocations. These checks happen outside timed execution; the
VM loop does not poll a deadline on every instruction.

These safeguards do not sandbox workload code or cap its total memory use.
Native and Python implementations can execute arbitrary code, and SST can loop,
recurse, or allocate repeatedly. Use disposable environments without secrets
for unfamiliar workloads. Direct calls to the fixture or validation entry points
need their own process supervision. Matching result metadata establishes which
run a report claims to describe; it cannot prove that PR-controlled measurements
are honest.

## Code organization

### CI scripts

`ci/index.cjs` exposes the GitHub workflow entry points for requesting a
run, publishing selected results, and posting a report comment. Helpers live
under `ci/`:

- `policy.cjs`: command parsing, PR authorization, and run provenance checks.
- `artifacts.cjs`: artifact selection and bounded reads of fixed archive files.
- `publication.cjs`: selected-result validation and non-forced Git publication.
- `comments.cjs`: PR association, report text, and comment posting.
- `results.cjs` and `report.cjs`: public schema validation and trusted rendering.

Privileged jobs load these modules from the trusted default-branch checkout
and read downloaded artifacts without executing their contents. Authorization
and provenance checks must precede publication, and branch updates must remain
non-forced. Run
`node --test benchmarks/ci/tests/ci.test.cjs` after changes; tests use local
archives and mocked GitHub APIs, without contacting GitHub or updating refs.

### Python support code

Run Python tools from this directory (or use `uv run --directory benchmarks`
from the repository root). The runner uses package-relative imports, and pyperf
launches its workers with `-m runner.python_bench` so they use the same package
layout. Direct execution of individual runner files is not supported.

The timed implementations live in `workloads/*/python.py`. The `runner/`
package contains the support code:

- `models.py`: typed workload, estimate, manifest, and comparison dataclasses.
- `contracts.py`: manifest validation, implementation loading, correctness checks.
- `validate_python.py`: child-process entry point for Python correctness checks.
- `validation.py`: type-dispatched input readers using `singledispatch`, without
  coercing strings or booleans into numeric inputs. Domain rules remain in contracts.
- `suite.py`: fingerprints and disposable merge-base checkouts.
- `processes.py` and `progress.py`: subprocess lifetime, log tails, and progress.
- `measurement.py`: `SuiteRunner` build, validation, and timing phases.
- `provenance.py`: environment capture and JSON serialization.
- `export.py`: reviewed result and numeric sample exports.
- `checks.py`: CI quality checks with local-only diagnostics.
- `estimates.py` and `report.py`: timing-tool ingestion and Markdown rendering.
- `run.py`: CLI options and comparison orchestration.

`python -m runner.python_bench` is the pyperf entry point. It times
`run(iterations, seed)` after setup and validation. The runner serializes its
result dataclasses into the versioned JSON artifact format.

The root `pyrightconfig.json` configures standard type checking for these scripts
and locates `benchmarks/.venv` for Pyright/Pylance. In VS Code, select that Python
interpreter if needed. Run `uv sync --directory benchmarks --locked` to install
the development tools. CI checks types and Ruff lint/formatting; formatting is
limited to `runner/` and `tests/python/` to leave measured implementations
unchanged.

### Rust support code

`src/lib.rs` exposes the repository-only support used by the validation binary
and Criterion driver:

- `src/workloads.rs`: contracts, bundle discovery, and correctness validation.
- `src/machine.rs`: frame preparation and the CPU/composite execution loop.
- `src/machine/program.rs`: full SST loading and immutable program metadata.
- `src/resolve.rs`: function indexing, local scope, signatures, and module assembly.
- `src/resolve/instructions.rs` and `values.rs`: exhaustive ISA lowering and
  typed constant encoding/string interning.
- `benches/workloads.rs`: native and SST program measurement registration.
- `benches/support/mod.rs`: measurement selection, Criterion configuration,
  and isolated instruction measurements.

`build.rs` generates the native registry in sorted bundle order. Fixture tests
live alongside the machine in `src/machine/tests.rs`. Setup, parsing, and
validation remain outside timing; route selection is compile-time and the
timed closures retain their black-box barriers and batched-reference setup.
