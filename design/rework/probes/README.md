# Step 1 reference probes

These fixtures run against the preserved draft, not the implementation branch.
They intentionally characterize bugs as well as working behavior. See the
[contract record](../contract.md) for desired behavior and migration ownership.

From the repository root:

```sh
python3 design/rework/probes/run.py --work-dir /tmp/vihaco-step1-reference
```

The destination must be empty. The runner archives the pinned reference commit,
checks compiler attribute acceptance, copies the integration/trybuild fixtures
into the archive, and runs Cargo offline. It needs the draft dependencies in
the Cargo cache. It leaves the archive available for inspection and editor
probes. It changes no branches or production sources. Omitting `--work-dir`
creates a fresh temporary directory. `--bless` explicitly rewrites the reference
diagnostic snapshots; review every changed `.stderr` before accepting it.

For editor checks, supply an installed rust-analyzer binary:

```sh
python3 design/rework/probes/editor.py /tmp/vihaco-step1-reference \
  --rust-analyzer /path/to/rust-analyzer
```

The script waits for workspace initialization and reports hover, definition,
and completion on the reference selector, both candidate selector views, and
a surface provenance route. Its JSON reports missing operations rather than
counting them as successes. The helper is an output-shape probe, not a finished
generalized dialect helper; recheck the implemented staging in Steps 5–7.

`compile_fail/raw-leaf-explicit-pattern.stderr` records an existing parser
derive panic. The replacement must diagnose that declaration without panic;
do not copy this snapshot into its acceptance tests.

## Step 3 bridge output shape

`step3-bridge.rs` is a standalone library fixture for the parent-import and
`pub use` bridge policy. Copy it to `src/lib.rs` in a temporary Cargo library
workspace and run:

```sh
python3 design/rework/probes/editor.py /tmp/vihaco-step3-editor \
  --source src/lib.rs \
  --probe 'selector=Add(inventory::__selectors::cpu::Add)' \
  --probe 'runtime=Option<inventory::__types::runtime::cpu::Add>' \
  --probe 'surface=Option<inventory::__types::surface::cpu::Add>' \
  --rust-analyzer /path/to/rust-analyzer
```

This probe checks the concrete bridge output shape. The owning macro steps
still need editor evidence for their actual first-expansion output.
