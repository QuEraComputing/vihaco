# AGENTS.md

This file provides guidance when working with code in this repository.
(`CLAUDE.md` is a symlink to this file.)

## Project

vihaco is a framework for building small **virtual machines** for hybrid
analog/digital quantum control orchestration. You define an *instruction set*,
the *components* that execute it, the *effects* they emit, and optionally a
*parser* for source text — all as ordinary Rust — then compose them into a
machine. A component boils down to one method:
`execute(&mut self, instruction, message) -> Result<Effects<Effect>>`.

It is a Cargo **workspace** of focused crates with no umbrella crate; depend on
what you need (`README.md` has the table and a worked example).

## Build & Test Commands

Tasks are also wrapped by [mise](https://mise.jdx.dev) (`mise run <task>`), but
the plain `cargo`/tool commands are equivalent:

```bash
cargo test --workspace --all-targets   # All tests EXCEPT doctests (mise run test)
cargo test --workspace --doc            # Guide + example doctests (mise run doctest)
cargo test test_name                    # Single test by name
cargo fmt --all                         # Format
cargo clippy --workspace --all-targets -- -D warnings   # Lint (mise run lint)
hawkeye check                           # SPDX license-header check (mise run license)
hawkeye format                          # Add/repair SPDX headers (mise run license-fix)
cd docs && pnpm install && pnpm dev     # Preview the docs site (Astro; prefer pnpm)
```

`--all-targets` deliberately **excludes** doctests — run `--doc` separately to
exercise the documentation (see `vihaco-doctests` under Architecture).

## Pre-commit Checklist

CI (`.github/workflows/`) runs these as separate jobs; run them before
committing. Tests run on Linux/macOS/Windows, so avoid platform-specific code.

```bash
cargo fmt --all -- --check                              # 1. Format
cargo clippy --workspace --all-targets -- -D warnings   # 2. Lint (warnings are errors)
cargo test --workspace --all-targets                    # 3. Test
cargo test --workspace --doc                            # 4. Doctests (docs can't drift)
hawkeye check                                            # 5. SPDX license headers
```

Every source file under `crates/**` carries a two-line SPDX header (see any
file's first two lines); `hawkeye format` adds it. Line-sensitive trybuild
fixtures under `tests/ui/` and `tests/compile_errors/` are intentionally
excluded from the header check.

## Architecture

### Crate layout

The workspace lives under `crates/`. The standard **virtual hardware
components** are the `vihaco-*` crates there (e.g. `vihaco-cpu`) — each is a
self-contained component you can use directly or copy as a starting point.

| Crate | Role |
|---|---|
| `vihaco` | The framework. Core types + the `module` / `syntax` / `runtime` layers. Re-exports the derives, so most code depends only on this crate. |
| `vihaco-cpu` | Standard computation and heap component. Use directly or as a reference component. |
| `vihaco-abi` | ISA vocabulary: `Instruction`/`Effects`, `Value`/`Type`, encoding + host-VM traits. Re-exports `#[derive(Instruction)]` via its `derive` feature. |
| `vihaco-abi-derive` | `#[derive(Instruction)]`. Consumed via `vihaco-abi`. |
| `vihaco-bytecode` | Binary/SST container format: headers, sections, instruction (de)coding, the `container/` codec. |
| `vihaco-module` | `LocalModule`, program metadata, lowering contexts, and assembly-style `Display`. |
| `vihaco-runtime` | Component and composite contracts, typed dispatch, stack protocols, and effect observation. Re-exports its derives via its `derive` feature. |
| `vihaco-runtime-derive` | The proc macros behind `#[derive(Message)]`, `#[dialect]`, `#[component]`, `#[composite]`, and `#[observe]`. Consumed via `vihaco-runtime`. |
| `vihaco-stdlib` | Program images, execution state, standard instructions, debugger, and observers. |
| `vihaco-syntax` | Typed SST parsing and module construction through lowering. |
| `vihaco-sst` | SST tooling: `Diagnostic`/`Report` (rendered via `codespan-reporting`); the `check` layers — `check_framing`, pluggable `SectionChecker`/`check_file`, the typed `ProgramChecker`/`check_program::<M>` for a composite machine, and `FileChecker` for one machine per section; the `Formatter`/`FileFormatter`; and the `Disassembler` (loaded `LocalModule`s back to SST via `Raise`). |
| `vihaco-parser` | The `Parse<'src>` and `SurfaceInstruction` traits plus lexical, primitive, and collection impls shared by the parser derive. |
| `vihaco-parser-derive` | `#[derive(Parse)]` — turns instruction, value, and type enums or structs into [chumsky](https://github.com/zesterer/chumsky) parsers via `#[syntax_class]` and `#[pattern]` (see `attr.rs`/`codegen.rs`). |
| `vihaco-doctests` | **Dev-only, not published.** `include!`s `docs/examples/*.rs` and runs every ` ```rust ` block in `docs/src/pages/guide/*.md` as a rustdoc doctest, so the public API and the docs can't drift. Editing the public API often requires updating these. |

### The mental model (what the macros generate)

- **Instruction** (`#[derive(Instruction)]`): an enum where each variant is an
  *opcode* and its tuple fields are the payload. The derive generates the
  bytecode traits in `traits/instruction.rs` — `OpCode` (opcode byte + `width`),
  `FromBytes`/`FromBytesWithOpcode`, `WriteBytes` (little-endian via
  `byteorder`) — plus the syntax descriptors in `instruction_syntax.rs`
  (`CanonicalInstructionSyntax`, sugar forms). Scalars (`u32/u64/i64/f64/bool/()`)
  already implement these traits, so they nest as instruction payloads.
- **Message** (`#[derive(Message)]`): runtime-supplied input that is *not*
  encoded in the instruction stream.
- **Effects<T>** (`effect.rs`): `None | One | Many(SmallVec)` — what `execute`
  returns. Composable via `append`/`extend`/`map`/`flat_map`;
  `expect_exactly_one_effect` is the common extractor.
- **Dialect and component** (`#[dialect]`, `#[component]`): a dialect declares
  instruction leaves and their syntax. A component declares the dialects it
  supports and implements `Execute<I>` for their leaves.
- **Composite** (`#[composite]`): a struct with `#[device]` fields. The macro
  generates runtime and syntax instruction aggregates and routes dispatch
  through the composite's authored `Step` implementations. A `#[program]`
  device supplies program execution state.

### Layers inside `vihaco`

- **Program data and execution** — `LocalModule` holds code and metadata,
  including functions, symbols, constants, strings, and source symbols.
  `Image` wraps the module. `Loadable` pairs it with a machine whose
  `ExecutionState` owns the program counter, locals, operands, and frames.
- **`syntax/`** — typed SST parsing and module construction. A section's
  `.text` is a flat instruction stream with `@name:` symbol definitions; the
  parser yields a `ParsedModule` with `code: Vec<I>` and a symbol table.
  SST is line-based like assembly: one statement per line (continued only
  inside open brackets), `;` comments, generated `;;` notes. The standard
  header is flat directives (`.file`, `.func`, `.entry`, `.const`,
  `.source_symbol`, `vihaco_module::module::ProgramDirective`) that
  `ProgramHeader` assembles into `ProgramMetadata`, deriving every table;
  function signatures live only in `.func` directives.
  Pattern-derived instruction enums implement `SurfaceInstruction`
  automatically. Consumers lower the parsed code against the prepared
  metadata and instruction context to build a runtime module.
- **Runtime contracts** — `Execute` defines instruction behavior and `Step`
  handles message preparation and effects at the composite boundary.
  `BuildMessage` selects a message recipe. `EffectSink` and `Observe` support
  effect handling, including the standard observers in `vihaco-stdlib`.
- **`value.rs`** — the runtime `Value` enum (`I64`/`U64`/`F64`/`Bool`/
  `FunctionRef`/`HeapRef`/interned `String`…) and `Type`.

`lib.rs` uses `extern crate self as vihaco;` so the derives can emit
`::vihaco::…` paths even within the `vihaco` crate itself.

## Key Conventions

- **Error handling:** this codebase uses **`eyre`** (`eyre::Result`,
  `eyre::eyre!`) throughout — *not* `anyhow`/`thiserror`. Match the surrounding
  code.
- **Rust edition:** every crate is **edition 2024** (rustc ≥ 1.85) EXCEPT
  `vihaco-parser` and `vihaco-parser-derive`, which are **edition 2021 with
  `rust-version = 1.75`** — keep their code within that MSRV.
- **Macro changes need trybuild coverage:** compile-fail behaviour is pinned by
  trybuild fixtures (`crates/vihaco/tests/ui/`,
  `crates/vihaco-parser-derive/tests/compile_errors/`). These are line-sensitive; when
  you change a diagnostic, update the matching `.stderr` (`TRYBUILD=overwrite
  cargo test`).

## Repository Tooling (ion)

This repo is managed by **ion** — it owns the agent setup, cloud
initialization, and the skills available here. `Ion.toml` is the source of
truth (`Ion.lock` pins versions/checksums):

- `[agents]` uses the `builtin:rust` template.
- `[skills]` enables `ion-cli` and `agents-update` (both `local`).
- `[options.targets]` emits skills to `.claude/skills`.

`.claude/` and `.agents/` are **generated by ion** — don't hand-edit files
there; change `Ion.toml` and let ion regenerate them (e.g. via the
`agents-update` skill). This `AGENTS.md` is the human-authored guidance and is
*not* ion-generated, so edit it directly.

## Docs

The site under `docs/` is an **Astro** app. Use **pnpm** for Node dependency
management (`pnpm install`, `pnpm dev`, `pnpm build`). Content lives in
`docs/src/pages/guide/*.md` and `docs/examples/*.rs`; both are compiled/run by
the `vihaco-doctests` crate, so keep code snippets in sync with the API (see
Architecture).

## Git Conventions

- **Conventional commits:** `feat:`, `fix:`, `docs:`, `test:`, `ci:`,
  `refactor:`, `perf:`, `build:`, `chore:`. Releases are automated from these
  via release-plz (see `RELEASING.md`) — don't `cargo publish` by hand.
- **Breaking changes:** use `feat!:` / `fix!:` (note the `!`) or a
  `BREAKING CHANGE:` footer.
