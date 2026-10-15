# Step 1 mechanism probes

These local fixtures exercise the proposed Rust relationships and bounded macro
expansions on the new implementation branch. They are not the implementation
of Steps 2–7 and must never be staged or committed. Production files are read
as dependencies; they are not patched by the probes.

The isolated Cargo workspace has a support model, two prototype attributes,
an external dialect producer, a reexport crate, and a consumer with renamed
dependencies. Marked instructions require unrestricted pub; runtime and surface fields share
their authored types. The producer uses the current real parser derive. The prototype
set attribute checks associations and emits paired types; parser composition
is exercised separately. The support model deliberately contains an invalid
unused generic constant to measure evaluation timing.

Run from the repository root:

```sh
python3 design/rework-2/probes/run.py
cargo test --offline --manifest-path design/rework-2/probes/roots/direct/Cargo.toml --target-dir /tmp/vihaco-rework2-step1-roots
cargo test --offline --manifest-path design/rework-2/probes/roots/facade/Cargo.toml --target-dir /tmp/vihaco-rework2-step1-roots
```

The runner compiles static fixtures in metadata and code-generation modes,
checks both exit status and the intended diagnostic, and writes logs plus
`results.json` under `/tmp/vihaco-rework2-step1-target/evidence`. Use
`--target-dir /tmp/another-directory` to choose another build location. It does
not generate Rust source or overwrite diagnostic snapshots. The Cargo JSON
artifact list selects libraries from the current build, avoiding stale hashes.
It also runs six actual Cargo check/build outcomes for unused, forced, and
generic reporting constants and twelve outcomes in an isolated feature graph.
The support and prototype macros are gated by nondefault `typed-declarations`.
The feature fixture checks the baseline without opt-in, absence of new APIs,
incomplete forwarding, explicit activation, and activation through another
dependency. These model local gating; production forwarding and marker/composite
switching remain implementation work. No compatibility macro is exported.

`missing-parser-root` and the two raw-parser cases intentionally characterize
current parser limitations, not acceptable diagnostics for the new dialect.
Field relations use parent-local usize indices and contain child types and
source names only. Hidden associated constants preserve exact field spelling;
the prototype set macro reads them through actual concrete parent types.
Generic relations/descriptions use literal indices. Index paths for duplicate
selection checks are tuples of standard unit-array types, without another
shared marker contract. The revised matrix also checks case-sensitive, raw,
Unicode, and reordered indices, and rejection of source aliases as field targets.
Dialect declarations generate
direct coverage bounds; no type-level instruction list is used. Source spelling
and local alias checks belong to owning macro steps rather than runtime consts.
`invalid-generic-report` must pass metadata checking and fail code generation;
unused invalid constants pass both. Successful compiler-only cases are type
proofs; runtime value assertions live in the consumer's tests.

Use the existing general LSP probe driver for editor evidence:

```sh
CARGO_TARGET_DIR=/tmp/vihaco-rework2-step1-editor CARGO_NET_OFFLINE=true \
python3 design/rework/probes/editor.py design/rework-2/probes \
  --rust-analyzer /path/to/rust-analyzer \
  --source consumer/src/lib.rs \
  --probe 'runtime_payload=Halt(chain::again::control::Halt)' \
  --probe 'import_chain=pub type RenamedAdd = chain::again::renamed::Add;' \
  --probe 'associated_surface=pub type AddSurface = <RenamedAdd as InstructionSpec>::Surface;'
```

The supplied rust-analyzer must be an installed binary rather than a rustup shim
for a missing component. Missing completion results remain recorded as missing.
The complete matrix and its production follow-ups are in
[the Step 1 report](../step-1-results.md).
