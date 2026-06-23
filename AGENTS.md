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
| `vihaco-cpu` | A ready-made CPU/host component: a stack machine with a `StepOutcome` control-flow effect. Use directly or as a reference component. |
| `vihaco-derive` | The proc macros behind `#[derive(Instruction/Message/Machine)]` and `#[component]` / `#[composite]` / `#[observe]`. Consumed via `vihaco`'s re-exports. |
| `vihaco-parser` | `#[derive(Parse)]` — turns an instruction enum into a [chumsky](https://github.com/zesterer/chumsky) parser via `#[head]`/`#[token]`/`#[delimiters]`/`#[parse_with]`/`#[delegate]` attributes (see `attr.rs`/`codegen.rs`). |
| `vihaco-parser-core` | The `Parse<'src>` trait + primitive impls shared by the parser derive. |
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
- **Component** (`#[component(instruction=, message=, effect=, outcome=)]`):
  wraps an `impl` block with an `execute(&mut self, inst, msg)` method into an
  impl of the `GeneratedComponent` trait (`runtime/generated.rs`).
- **Composite / Machine** (`#[composite]`, which is `#[derive(Machine)]` plus
  field-attr stripping): a struct of devices. Each `#[device(code, alias=...)]`
  field becomes a variant of a generated `<Name>Instruction` enum (one opcode
  per device); one field may be `#[program]` to delegate `ProgramCounter`. The
  derive emits a `GeneratedMachine` impl exposing `CompositeMetadata` (device
  codes + source-symbol aliases). See `vihaco-derive/src/derive_machine.rs`.

### Layers inside `vihaco`

- **`module.rs`** — `Module<I, V, Ty, Info>`, the loadable program: `code`,
  `functions`, `labels`, `constants`, `strings`, `source_symbols`, plus
  pluggable `extra` metadata. Its `Display` prints an assembly-style dump
  (`.text`/`.const`/`.string`/`.machine`/`.feature`). `loader.rs` wraps a
  `Module` + program counter as a `ProgramLoader`.
- **`syntax/`** — a **two-pass** parse→resolve pipeline. The parser yields a
  `ParsedModule`/`ParsedFunction` of `BodyItem`s that are either `Direct`
  (already-typed instruction) or `Raw` (untyped source form awaiting
  expansion). Each instruction set implements the `Resolve` trait to lower
  `Vec<BodyItem<I>>` into `Vec<I>`.
- **`runtime/` + `traits/`** — the host-VM interfaces a CPU-like component
  implements: `ProgramCounter`, `StackMemory`, `StackFrame`, `FrameMemory`,
  `GetProgramGlobal`, `Stdout` (`traits/machine.rs`), plus `Reset` and the
  `EffectSink` / `Observe` machinery (`#[observe]`, `observer/stdio.rs`).
- **`value.rs`** — the runtime `Value` enum (`I64`/`U64`/`F64`/`Bool`/
  `FunctionRef`/`HeapRef`/interned `String`…) and `Type`.

`lib.rs` uses `extern crate self as vihaco;` so the derives can emit
`::vihaco::…` paths even within the `vihaco` crate itself.

## Key Conventions

- **Error handling:** this codebase uses **`eyre`** (`eyre::Result`,
  `eyre::eyre!`) throughout — *not* `anyhow`/`thiserror`. Match the surrounding
  code.
- **Rust edition:** `vihaco`, `vihaco-cpu`, `vihaco-derive`, `vihaco-doctests`
  are **edition 2024** (rustc ≥ 1.85). `vihaco-parser` and `vihaco-parser-core`
  are **edition 2021 with `rust-version = 1.75`** — keep their code within that
  MSRV.
- **Macro changes need trybuild coverage:** compile-fail behaviour is pinned by
  trybuild fixtures (`crates/vihaco/tests/ui/`,
  `crates/vihaco-parser/tests/compile_errors/`). These are line-sensitive; when
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
