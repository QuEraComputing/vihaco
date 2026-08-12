# Crate Split Plan

> **Status:** executed on branch `refactor/crate-split` (Waves 0–4 landed; full gate green).
> **Owner:** (assign)
> **Last revised:** 2026-07-30
>
> This is a durable design doc, deliberately kept **out of** `docs/src/pages/`
> (the Astro site) and **out of** the `vihaco-doctests` include list, so nothing
> here is compiled or run as a doctest. Edit it freely as the plan evolves.

## 1. Purpose & goals

The `vihaco` crate has grown into a monolith that mixes the ISA vocabulary, the
bytecode/SST container format, the loadable module model, the runtime/component
machinery, and the text frontend. The internal module graph is already cleanly
layered — the problem is that it all lives in one crate, so every change forces
a rebuild of everything and there are no enforced layering boundaries.

Goals, in priority order:

1. **Keep the public API as identical as possible.** The facade crate `vihaco`
   must keep re-exporting every symbol at its current path. Downstream code
   (including `vihaco-cpu` and any external users) should compile unchanged.
2. **Do not change runtime behavior or the parsing logic** in this work. This is
   a *relocation + crate-boundary* refactor, not a logic rewrite. (The parser
   *idiomaticity* cleanup in §8 is sequenced as separate, later work.)
3. **Make future refactoring easier** by cutting along the layering seams we
   already have, and by giving each derivable-trait crate an explicit,
   conventionally-named `-derive` companion.

Non-goals for this pass: hiding `chumsky` behind an abstraction, sharpening
macro diagnostics, or any change that touches the pinned `trybuild` `.stderr`
fixtures. Those are called out as follow-ups.

## 2. Decisions locked in

- **Derive crates: per-crate companions.** Each trait-owning crate gets its own
  `<crate>-derive` proc-macro sibling (the serde / zerocopy model), rather than
  one catch-all derive crate. See §5.
- **Parser crates: renamed serde-style.** `vihaco-parser-core` → **`vihaco-parser`**
  (the traits + primitives), and today's `vihaco-parser` → **`vihaco-parser-derive`**
  (the `#[derive(Parse)]` macro). See §8.

## 3. Current state

### 3.1 Workspace today

```
crates/
  vihaco              framework monolith (core + module + syntax + runtime + binary)
  vihaco-cpu          ready-made CPU component (unchanged by this plan)
  vihaco-derive       ALL framework proc-macros (Instruction, Message, component, composite, observe, machine)
  vihaco-parser       #[derive(Parse)] proc-macro  (misnamed: it is a derive crate)
  vihaco-parser-core  Parse / SurfaceInstruction traits + primitives + container codec
  vihaco-doctests     dev-only; runs docs + examples (unchanged)
```

### 3.2 Internal layering inside `vihaco` (verified)

Foundation (no internal deps): `effect`, `frame`, `metadata`, `instruction_syntax`,
`color`, the encoding/instruction/event_sink traits, `value`/`program` (→ traits).
Mid: `binary` (→ traits, `vihaco_parser_core::container`), `module` (→ color).
Upper: `loader` (→ binary + module + host-VM traits), `runtime` (→ metadata,
traits, module, effect), `syntax` (→ binary, parser). Top: `observer` (→ runtime).

### 3.3 Two hard constraints

1. **Host-VM traits reach upward.** `traits/machine.rs` (`ProgramCounter`,
   `StackMemory`, `StackFrame`, `FrameMemory`, `GetProgramInfo`, `Stdout`)
   imports `frame::Frame`, `module::FunctionInfo`, and `ConstantId` (from
   `binary`) — `traits/machine.rs:6,8`. So these traits belong *above* `module`
   and `bytecode`; the `traits` module must be split across crates.
2. **Derive-emitted paths.** Generated code currently hard-codes `::vihaco::…`
   paths (crate root + `instruction`, `metadata`, `loader`, `runtime`,
   `__private`). Inside the workspace, `extern crate self as vihaco;`
   (`lib.rs:4`) is what makes those resolve. Splitting changes where generated
   code lives, so we address this head-on in §5 (root resolution).

Complete set of `::vihaco::…` paths emitted by the current derive:

- Crate root: `Effects`, `GeneratedComponent`, `Observe`, `Instruction`,
  `CompositeMetadata`, `BytecodeSectionView`, `SstSectionView`.
- `instruction::{OpCode, FromBytes, FromBytesWithOpcode, WriteBytes}`
- `metadata::{DeviceMetadata, SourceSymbolAliasMetadata}`
- `loader::{LoadOwnBytecodeSection, LoadBytecodeSection, LoadSstProgram, LoadSstSubtree}`
- `runtime::Message`
- `__private::GeneratedMachine`

## 4. Target crate graph

Eleven framework crates (plus unchanged `vihaco-cpu`, `vihaco-doctests`):

| Crate | Role | Depends on |
|---|---|---|
| **vihaco-abi** | ISA vocabulary: `Effects`, `Value`/`Type`, `Frame`, `metadata`, `instruction_syntax`, encoding/instruction/event_sink traits, `Reset`. Re-exports its derive behind `derive` feature. | byteorder, smallvec, chumsky\* |
| **vihaco-abi-derive** | `#[derive(Instruction)]` | syn, quote, proc-macro2, proc-macro-crate |
| **vihaco-bytecode** | `binary/*` (headers, sections, contexts, `decode_instruction_stream`) **+ absorbed container codec** | vihaco-abi, byteorder, chumsky |
| **vihaco-module** | `color`, `module`, host-VM traits (`host.rs`), `loader` | vihaco-abi, vihaco-bytecode, colored |
| **vihaco-runtime** | `runtime/*`, `__private`. Re-exports its derive behind `derive` feature. | vihaco-abi, vihaco-bytecode, vihaco-module |
| **vihaco-runtime-derive** | `#[derive(Message)]`, `#[derive(Machine)]`/`#[composite]`, `#[component]`, `#[observe]` | syn, quote, proc-macro2, proc-macro-crate |
| **vihaco-stdlib** | Standard-library components and observers (`observer/*`). | vihaco-runtime, eyre |
| **vihaco-parser** | *(renamed from vihaco-parser-core)* `Parse`, `SurfaceInstruction`, primitive/lexical/collection impls. Re-exports its derive behind `derive` feature. | chumsky |
| **vihaco-parser-derive** | *(renamed from vihaco-parser)* `#[derive(Parse)]` | syn, quote, proc-macro2, eyre |
| **vihaco-syntax** | `syntax/*` (SST parse, `Resolve`, `Parsed*` types) | vihaco-abi, vihaco-bytecode, vihaco-parser (+ vihaco-parser-derive for tests) |
| **vihaco** | **facade**: pure re-exports at identical paths; keeps `public_api_tests`. | all of the above |

`*` `chumsky` lands in `vihaco-abi` only because `value.rs`'s `FromText` impl
uses it today. Removing that (per §8) later drops chumsky from `abi`.

Dependency DAG:

```
   vihaco-abi ◄──────── vihaco-abi-derive
      ▲   ▲
      │   └────────────── vihaco-bytecode (absorbs container codec)
      │                        ▲     ▲
      │                        │     └───────────── vihaco-syntax ──► vihaco-parser ◄── vihaco-parser-derive
      │                        │                          ▲
      │                   vihaco-module                   │
      │                        ▲                           │
      └───────────────── vihaco-runtime ◄── vihaco-runtime-derive
                               ▲   ▲   ▲   ▲
                               └───┴───┴───┴──────── vihaco (facade) ──► (re-exports every crate above)
```

## 5. Derive-crate convention (the core of this revision)

We follow the established Rust convention for a facade + sub-crates + derive
setup (serde, zerocopy, bevy): **the proc-macro lives in a `-derive` sibling of
the trait crate, the trait crate re-exports it behind a `derive` feature, and
generated code is rooted at a crate the *consumer actually depends on*, resolved
at macro-expansion time.**

### 5.1 Which macro moves where

Split the current `vihaco-derive` by the trait each macro targets:

| Macro | Backing file(s) today | New home | Generated code roots at |
|---|---|---|---|
| `#[derive(Instruction)]` | `derive_instruction.rs` | **vihaco-abi-derive** | `vihaco-abi` (self-contained: OpCode/FromBytes/WriteBytes/instruction_syntax all in abi) |
| `#[derive(Message)]` | `derive_message.rs` | **vihaco-runtime-derive** | `vihaco-runtime` |
| `#[component]` | `attr_component.rs` | **vihaco-runtime-derive** | `vihaco-runtime` |
| `#[composite]`/`#[derive(Machine)]` | `attr_composite.rs`, `machine` in `lib.rs` | **vihaco-runtime-derive** | `vihaco-runtime` (+ symbols re-exported *through* runtime — see 5.3) |
| `#[observe]` | `attr_observe.rs` | **vihaco-runtime-derive** | `vihaco-runtime` |
| `#[derive(Parse)]` | `vihaco-parser/{lib,attr,codegen}.rs` | **vihaco-parser-derive** | `vihaco-parser` |

`common.rs` holds shared helpers. If both `-derive` crates need it, factor it
into a small **non-proc-macro** support crate (working name
`vihaco-derive-utils`) that both depend on — do **not** duplicate proc-macro
crates. Confirm `common.rs`'s contents during execution; if it is trivial and
only one crate needs it, just move it there.

### 5.2 Root resolution (makes both facade and direct-dep usage work)

Generated code must reference a crate the consumer actually has in its
dependency graph. A user might depend on the **facade** (`vihaco`) *or* on a
**specific crate** (`vihaco-abi`). Emitting a fixed `::vihaco_abi::…` would break
facade-only users (they have no `vihaco_abi` extern name); emitting a fixed
`::vihaco::…` would break "depend on what you need" users.

**Resolution:** each `-derive` crate uses the `proc-macro-crate` crate
(`proc_macro_crate::crate_name`) to detect, at expansion time, whether the
downstream depends on the facade or the specific trait crate, and emits paths
rooted accordingly. Provide the conventional escape hatch as well — a
`#[vihaco(crate = ::some_path)]` (and `#[parse(crate = ...)]`) override — for
re-export / renamed-dependency scenarios. This is exactly what serde's
`#[serde(crate = "...")]` and bevy's macro-utils do.

Invariant that keeps this sound: **every root a derive can resolve to must
expose the same sub-paths.** Concretely, both `vihaco::metadata::DeviceMetadata`
and `vihaco_runtime::metadata::DeviceMetadata` must exist. The facade already
mirrors sub-crate paths; §5.3 makes `vihaco-runtime` mirror the rest.

### 5.3 `vihaco-runtime` must re-export what its derive emits

`#[composite]` is cross-cutting: generated code touches `Effects` (abi),
`metadata::*` (abi), `BytecodeSectionView`/`SstSectionView` (bytecode),
`loader::Load*` (module), `CompositeMetadata`/`GeneratedComponent`/`__private::GeneratedMachine`
(runtime), and the `Instruction` derive (abi-derive). To keep `vihaco-runtime`
the single root for its derive, `vihaco-runtime` re-exports all of these under
its own namespace:

```rust
// vihaco-runtime/src/lib.rs (sketch)
pub use vihaco_abi::{Effects, metadata};                 // ::vihaco_runtime::Effects, ::vihaco_runtime::metadata::*
pub use vihaco_bytecode::{BytecodeSectionView, SstSectionView};
pub use vihaco_module::loader;                            // ::vihaco_runtime::loader::Load*
pub use vihaco_runtime_derive::{Message, Machine, component, composite, observe}; // feature = "derive"
pub mod __private { /* GeneratedMachine */ }
```

This is the rule "**each derive emits paths only into its paired crate**"
(serde's model) applied here; the paired crate re-exports downward as needed.

### 5.4 `derive` feature (serde convention)

Each trait crate gates its derive re-export behind a `derive` feature:

```toml
# vihaco-abi/Cargo.toml
[features]
default = ["derive"]      # default-on to preserve today's "just works" API
derive  = ["dep:vihaco-abi-derive"]
```

Default-on preserves the current behavior (where `vihaco::Instruction` is always
available). Users who want the trait crate without the proc-macro dep can opt out
with `default-features = false`.

## 6. File-by-file move table (framework split)

`use crate::…` rewrites are internal path edits, not API changes. Sub-crate test
modules that invoke derives take the facade (or the specific derive crate) as a
**dev-dependency** — Cargo permits dependency cycles through `dev-dependencies`.

| Source (`crates/vihaco/src/…` unless noted) | → Destination | Path rewrites | Notes |
|---|---|---|---|
| `effect.rs` | vihaco-abi | — | `Effects<T>` |
| `frame.rs` | vihaco-abi | — | `Frame` |
| `metadata/{mod,device,scheduler}.rs` | vihaco-abi | — | |
| `instruction_syntax.rs` | vihaco-abi | — | |
| `program.rs` + `value.rs` | vihaco-abi | traits paths stay in-crate | keep `#[path]` mount so `program::value` survives |
| `traits/{encoding,event_sink,instruction}.rs` + `Reset` (from `traits/mod.rs`) | vihaco-abi | `super::…` unchanged | new `traits/mod.rs` re-exports these four only |
| `binary/{mod,context,file,format,section,tests}.rs` | vihaco-bytecode | `crate::traits::*`→`vihaco_abi::traits::*`; `vihaco_parser_core::container::*`→`crate::container::*` | `tests.rs`: `vihaco` dev-dep |
| `vihaco-parser-core/src/container/*` | vihaco-bytecode (`src/container/`) | internal | folds codec into its only consumer (§8 step 2) |
| `color.rs` | vihaco-module | — | `#[macro_export] show!/show_instruction!` verified relocation-safe (no `$crate`/`crate::` paths) |
| `module.rs` | vihaco-module | color path stays in-crate | as `pub mod module` |
| `traits/machine.rs` | vihaco-module (`src/host.rs`) | `super::Instruction`→`vihaco_abi::traits::Instruction`; `crate::{ConstantId,frame::Frame,module::FunctionInfo}`→`vihaco_bytecode::ConstantId`, `vihaco_abi::frame::Frame`, `crate::module::FunctionInfo` | host-VM traits; reason they cannot live in abi |
| `loader.rs` | vihaco-module | `crate::binary::*`→`vihaco_bytecode::*`; `crate::program::*`→`vihaco_abi::program::*`; host traits→`crate::host::*` | widest fan-in |
| `runtime/{mod,generated,marker,observe}.rs` | vihaco-runtime | `crate::metadata`→`vihaco_abi::metadata`; `crate::module::LocalModule`→`vihaco_module::module::LocalModule`; `crate::Effects`→`vihaco_abi::Effects` | + re-exports from §5.3 |
| `observer/{mod,stdio}.rs` | vihaco-stdlib | `#[observe]` from `vihaco-runtime-derive` through `vihaco-runtime` | the one non-test derive user; no longer forces the facade cycle |
| `__private.rs` | vihaco-runtime | `crate::runtime::CompositeMetadata`→`crate::CompositeMetadata` | `GeneratedMachine` |
| `syntax/{mod,types,parse,resolve}.rs` | vihaco-syntax | `crate::{SstHeader,SstSectionView}`→`vihaco_bytecode::*`; `crate::SurfaceInstruction`→`vihaco_parser::SurfaceInstruction` | tests: `vihaco-parser-derive` + `vihaco` dev-deps |
| `macros/mod.rs`, `instruction.rs`, `machine.rs`, `lib.rs` (+ `public_api_tests`) | vihaco (facade) | rewritten as re-exports | see §7 |
| `crates/vihaco-derive/src/derive_instruction.rs` (+ needed `common.rs`) | vihaco-abi-derive | repoint `::vihaco::instruction::*`→resolved-root (§5.2) | |
| `crates/vihaco-derive/src/{derive_message,attr_component,attr_composite,attr_observe}.rs` + `machine`/`common.rs` | vihaco-runtime-derive | repoint `::vihaco::*`→resolved-root (§5.2) | |
| `crates/vihaco-parser/*` | **rename** dir → `crates/vihaco-parser-derive/` | update `name` in Cargo.toml + workspace member | generated `::vihaco_parser_core::*` → `::vihaco_parser::*` |
| `crates/vihaco-parser-core/*` | **rename** dir → `crates/vihaco-parser/` (minus `container/`) | update `name` + `[workspace.dependencies]` key + all refs | |

## 7. Facade `vihaco` design

`vihaco/src/lib.rs` keeps `extern crate self as vihaco;` (harmless) and re-mounts
every current module/symbol path, sourced from the sub-crates. Whole-module
re-exports (`pub use crate_or_module::submodule;` / `pub use crate as alias;`)
preserve every sub-path, so `vihaco::syntax::parse::block_i64_flat`,
`vihaco::program::value::Type`, etc. all still resolve.

```rust
// module tree — identical public paths
pub use vihaco_abi::{effect, frame, instruction_syntax, metadata, program};
pub use vihaco_module::{color, loader, module, show, show_instruction};
pub use vihaco_syntax as syntax;
pub use vihaco_runtime::{observer, runtime};
#[doc(hidden)] pub use vihaco_runtime::__private;

pub mod instruction { pub use vihaco_abi::traits::{FromBytes, FromBytesWithOpcode, Instruction, OpCode, WriteBytes}; }
pub mod machine     { pub use vihaco_module::host::{FrameMemory, GetProgramInfo, ProgramCounter, StackFrame, StackMemory, Stdout}; }
#[doc(hidden)] pub mod traits { pub use vihaco_abi::traits::*; pub use vihaco_abi::Reset; pub use vihaco_module::host::*; }
pub mod macros { pub use vihaco_abi::Instruction; pub use vihaco_runtime::{Message, component, composite, observe}; /* Machine */ }

// crate-root re-exports — mirror current lib.rs
pub use vihaco_bytecode::{ /* the 20-symbol binary group */ };
pub use vihaco_abi::{Effects, Type, Value};
pub use vihaco_abi::instruction_syntax::{ /* Canonical…, Sugar… */ };
pub use vihaco_module::loader::{LoadBytecodeSection, /* … */ ProgramImage};
pub use vihaco_abi::Instruction;
pub use vihaco_runtime::{CompositeMetadata, EffectSink, GeneratedComponent, Message as MessageMarker, Observe, expect_exactly_one_effect, Message, component, composite, observe};
pub use vihaco_abi::{Reset, FromBytes, FromText};
pub use vihaco_module::host::GetProgramInfo;
pub use vihaco_parser::SurfaceInstruction;
```

`public_api_tests` (currently `lib.rs:52-181`) stays in the facade and is the
**acceptance gate**: if it compiles unchanged, the public surface is intact.

## 8. Parser reorganization

Verdict from analysis: the **traits-crate vs derive-crate split is correct** (a
`proc-macro=true` crate can't export the runtime traits). The problems are
naming, mis-homed code, and duplication — not the boundary itself.

Safe pass (no behavior change; two steps merge into the split above):

1. **Workspace-pin chumsky** (+ `byteorder`, `eyre`): move to root
   `[workspace.dependencies]`, flip all crates to `{ workspace = true }`. Zero
   code change; removes the 5-way independent `chumsky = "0.10"` pin. *Do first.*
2. **Move the container codec into `vihaco-bytecode`** (already in §6). Its only
   consumer is `vihaco::binary`; this un-mixes `vihaco-parser` (née
   `-core`) so it is purely `Parse` + `SurfaceInstruction` + primitives.
3. **Rename the parser crates** (serde-style, per §2): `vihaco-parser-core` →
   `vihaco-parser`, `vihaco-parser` → `vihaco-parser-derive`. Update generated
   paths `::vihaco_parser_core::*` → `::vihaco_parser::*`, the workspace
   member list, and the `[workspace.dependencies]` keys.
4. **Dedupe SST primitives**: have `vihaco-syntax` reuse `vihaco-parser`'s
   `QuotedString`/`i64` parsers instead of re-implementing `string_literal` and
   `block_i64_*`.
5. **Delete dead attribute code** in `vihaco-parser-derive` (`attr.rs`
   `#[token]`/`#[delimiters]`/`#[delegate]`/`DelimiterAttrs`/`string_attr` — parsed
   then discarded, not even registered helper attributes).
6. **Mechanical derive cleanup** (`codegen.rs`, 1049 lines): split into
   `model`/`dsl`/`validate`/`emit`/`expand`; deduplicate enum/struct emission;
   replace the pattern-string round-trip with direct IR; thread `syn::Result`
   instead of `eyre`. Keeps all 24 `trybuild` fixtures byte-identical.

Deeper / out of scope now (flagged): hide chumsky behind
`vihaco_parser::parse_str::<T>()` + error-type aliases so consumers stop
importing chumsky; eventual sealed `Parser` newtype (a real API change); span
sharpening (step that intentionally rewrites `.stderr` fixtures — its own PR).

## 9. Execution order (serial reference)

> For the **parallelized** version we actually execute, see §12. This section is
> the serial baseline it is derived from.

Each step must build and pass `cargo test --workspace --all-targets`,
`--doc`, `cargo clippy -- -D warnings`, and `hawkeye check` before the next.
Bottom-up so each crate compiles against already-extracted deps.

1. **Workspace-pin chumsky** (§8.1) — trivial, unblocks everything.
2. **vihaco-abi** + **vihaco-abi-derive** — leaf vocabulary + Instruction derive; wire the `derive` feature and `proc-macro-crate` root resolution here first (smallest surface to prove §5).
3. **vihaco-bytecode** — includes absorbing the container codec (§8.2).
4. **rename parser crates** (§8.3) — `-core`→`vihaco-parser`, derive→`vihaco-parser-derive`.
5. **vihaco-module** — color + module + `host.rs` + loader.
6. **vihaco-runtime** + **vihaco-runtime-derive** — runtime + observer + `__private`; the §5.3 re-exports.
7. **vihaco-syntax**.
8. **Gut `vihaco` to the facade** (§7); verify `public_api_tests` + full CI green.
9. **Parser idiomaticity cleanup** (§8.4–8.6) — after the split settles.

**Sub-agent orchestration.** Steps 2–7 are naturally one-crate-per-agent, each
in an isolated git worktree, each responsible for: moving files, rewriting
`use` paths, adding facade/runtime re-exports, and proving `cargo build -p
<crate>` + tests. Because the steps are strictly bottom-up (each depends on the
prior crate existing), run them as a **pipeline**, not a barrier — but note that
each agent's output is a real code change on a shared branch, so serialize the
merges. A good harness: one driver agent that extracts crate N, runs the gate,
commits, then hands the updated tree to the agent for crate N+1. Do **not**
fan out steps 2–7 in parallel against the same tree — they edit overlapping
files (`vihaco/src/lib.rs` shrinks at every step).

## 10. Risks & open questions

- **`proc-macro-crate` MSRV / edition.** The `-derive` crates and
  `vihaco-parser` are edition 2021 / `rust-version = 1.75`. Confirm
  `proc-macro-crate` supports that floor; if not, pin an older compatible
  version or gate root-resolution behind manifest inspection.
- **`common.rs` sharing.** Decide during step 2/6 whether to introduce
  `vihaco-derive-utils` (non-proc-macro) or move/duplicate. Inspect its contents
  first.
- **Dev-dependency cycles.** `vihaco-bytecode`/`vihaco-syntax` tests that use
  derives depend on `vihaco`(-derive) as a **dev-dependency**. Verify Cargo is
  happy with each specific cycle (it allows dev-dep cycles, but confirm per
  crate).
- **`chumsky` in `vihaco-abi`.** Accepted for now (value's `FromText`). Revisit
  after §8's chumsky-hiding work; may let `abi` drop chumsky entirely.
- **`release-plz` / versioning.** New crates need `[workspace.dependencies]`
  entries and version alignment; renamed crates (`vihaco-parser*`) are a
  breaking change for anyone depending on them directly — coordinate a version
  bump and note it in the changelog (`feat!:`).
- **`vihaco-cpu`, `vihaco-doctests`, docs snippets** depend on the facade; they
  should be unaffected, but they are the real-world regression check — keep them
  in the CI gate at every step.

## 11. Alternatives considered

- **Single `vihaco-derive` for all framework macros** (emitting `::vihaco::…`
  via the facade). Fewer crates and no `proc-macro-crate` needed, but the derive
  stays coupled to the facade, which forces `runtime`/`observer` to remain in
  the facade crate (the cycle constraint of §3.3). Rejected in favor of the
  per-crate companions chosen in §2, which decouple cleanly and match the
  "depend on what you need" philosophy.
- **Keep parser crate names as-is.** Rejected: `vihaco-parser` being a derive
  crate with no `-derive` suffix is exactly the naming problem this revision
  fixes.
- **Leave `container` in `vihaco-parser(-core)`.** Rejected: it is a file-format
  codec with a single consumer (`vihaco::binary`) and no relation to parser
  combinators; it belongs in `vihaco-bytecode`.

## 12. Parallel execution plan (sub-agents + worktrees)

### 12.1 How much parallelism is actually available

The runtime crates form a near-linear dependency chain
(`abi → bytecode → module → runtime → facade`), and a chain does not
parallelize. The only genuinely independent pairs are **{abi, parser}** and
**{module, syntax}**. So we do not fan out all crates at once; we run
**dependency waves** — parallel *within* a wave, serial *across* waves. Peak
width is 2 agents. This is a modest but real speedup on a 6-deep critical path,
and it keeps every intermediate state buildable.

### 12.2 Driver / agent responsibility split (avoids merge conflicts)

The one file every extraction would otherwise touch is `crates/vihaco/src/lib.rs`
(and, to a lesser degree, the root `Cargo.toml` members list). To keep worktree
branches mergeable:

- **Agents own** (inside their worktree): creating the new crate directory and
  its `Cargo.toml`; `git mv`-ing the listed files out of `crates/vihaco/src`
  into the new crate; rewriting intra-file `use crate::…` paths per §6; adding
  the two-line SPDX header to any *new* file (`hawkeye format`); and proving
  **`cargo build -p <new-crate>`** (plus its own unit tests) is green. Agents
  **must not** edit `crates/vihaco/src/lib.rs`. Adding their crate to the root
  `Cargo.toml` `members` list is fine (union-merges trivially).
- **The driver (this session) owns**: merging each agent's branch into the
  single integration branch; resolving the (trivial) root `Cargo.toml`
  conflicts; **incrementally rewriting `crates/vihaco/src/lib.rs`** after each
  wave so the *whole workspace* stays green (façade re-exports the newly
  extracted crate, drops the now-moved `mod`s); running the full CI gate; and
  committing the wave. It is expected that inside an agent's worktree `cargo
  build -p vihaco` is temporarily red (moved files, stale `lib.rs`) — that is the
  driver's to fix at integration, not the agent's.

### 12.3 Git & worktree mechanics

- Single integration branch: **`refactor/crate-split`** off `main`. Everything
  lands here; **one PR at the end** (per the explicit "no multiple PRs" rule).
- Wave 0 (driver): create the branch, apply the `chumsky`/`byteorder`/`eyre`
  workspace pin (§8.1), commit. This is the base every worktree branches from.
- Each subsequent wave: spawn its agent(s) with `isolation: "worktree"`, each
  branched from the **current tip** of `refactor/crate-split`. When the wave's
  agents finish, the driver merges their branches in, updates `lib.rs`, runs the
  gate, and commits. Only then does the next wave's worktree get created — so
  each wave sees its dependencies already present.
- Never fan out dependent waves against the same tree in parallel; they edit
  overlapping files. Parallelism is strictly intra-wave.

### 12.4 The waves

| Wave | Runs | Crates | Notes |
|---|---|---|---|
| 0 | driver | — | branch + workspace-pin chumsky/byteorder/eyre |
| 1 | **2 agents ∥** | (a) `vihaco-abi` + `vihaco-abi-derive`; (b) rename `vihaco-parser-core`→`vihaco-parser` and `vihaco-parser`→`vihaco-parser-derive` | agent (a) also wires the `derive` feature + `proc-macro-crate` root resolution — the pattern every later derive reuses. Agent (b) leaves `container/` in place (bytecode takes it in wave 2). |
| 2 | 1 agent | `vihaco-bytecode` | extract `binary/*`; absorb `container/*` out of `vihaco-parser` |
| 3 | **2 agents ∥** | `vihaco-module`; `vihaco-syntax` | module = color+module+`host.rs`+loader; syntax = `syntax/*` |
| 4 | 1 agent | `vihaco-runtime` + `vihaco-runtime-derive` | runtime+observer+`__private`; §5.3 re-exports |
| 5 | driver | `vihaco` façade | final `lib.rs` rewrite to §7; whole-workspace green |

### 12.5 CI gate (run by driver after every wave merge)

```
cargo build --workspace
cargo test  --workspace --all-targets
cargo test  --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
hawkeye check
```

`trybuild` fixtures under `tests/ui/` and `tests/compile_errors/` are
line-sensitive and excluded from the SPDX check; if a diagnostic path changes
during the parser rename, regenerate with `TRYBUILD=overwrite` and review the
diff (should be limited to `vihaco_parser_core` → `vihaco_parser`).

### 12.6 Rollback

Each wave is one squash-merge commit on `refactor/crate-split`. If a wave fails
the gate and can't be fixed quickly, `git reset --hard` to the previous wave's
commit and re-run that wave's agent(s) with a corrected brief. The branch is
never force-pushed until the PR is opened.
