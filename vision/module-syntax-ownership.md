# Module syntax rewrite ownership map

This map records the initial disjoint ownership boundaries for the parallel
implementation tracks in `module-syntax-plan.md`. Agents must inspect the
current state before editing and report any required boundary change before
touching another track's implementation files.

## Agent A — `vihaco-syntax` contract

- `crates/vihaco-syntax/src/lib.rs`
- `crates/vihaco-syntax/src/types.rs`
- `crates/vihaco-syntax/src/parse.rs`
- `crates/vihaco-syntax/src/resolve.rs`
- focused tests contained in the files above

## Agent B — component instruction-set syntax API

- component-side public API files under `crates/vihaco-runtime/src/`
- component derive API/codegen files under `crates/vihaco-runtime-derive/src/component*`
- component/parser-focused tests under `crates/vihaco-runtime/tests/` and
  `crates/vihaco/tests/component_macro.rs`

Agent B must not edit `crates/vihaco-runtime-derive/src/composite/`.

## Agent C — loader capability rename

- `crates/vihaco-module/src/loader.rs`
- `crates/vihaco-module/src/lib.rs`
- loader-focused tests owned by `crates/vihaco-module`

Agent C may update direct in-repository consumers only where necessary to
complete the rename, coordinating conflicts with the coordinator rather than
changing generated composite behavior.

## Later sequenced ownership

- Agent D: `crates/vihaco-runtime-derive/src/composite/` syntax/codegen and
  dedicated macro fixtures; starts after A and B.
- Agent E: composite header syntax/resolution and metadata builder seams;
  starts after the contract and codegen interfaces are available.
- Agent F: generated resolver/loading integration, primarily composite
  `codegen.rs` and generated SST loading tests; starts after A, C, and D.
- Agent G: nested generated loading and nested fixtures; starts after C and F.
- Agent H: semantic lowering/resolution tests and implementation seams; starts
  after F.
- Agent I: cross-crate regression coverage; starts after F, G, and H.
- Agent J: demo, guide, and companion-plan migration; starts after I.

The coordinator owns conflict resolution, public API consistency, and all
integration-gate verification.
