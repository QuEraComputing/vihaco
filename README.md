# vihaco

A virtual ISA and machine framework for orchestrating hybrid analog/digital
quantum control. Define instruction sets, components, effects, and
pattern-derived source syntax as ordinary Rust — then compose them into a
machine.

[![CI](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml/badge.svg)](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it is

vihaco is a framework for building small virtual machines. You define

- reusable **components** and their instruction products with `component!`;
- one `Execute<I>` implementation per product, with typed messages and effects;
- **composite routes** with `composite!`;
- executable composite **surface syntax and program loading** with the parser
  derives; and
- (optionally) standalone **SST source syntax** with the parser derives,

all as ordinary Rust. A component step is
`execute(&instruction, message) -> StepResult<effect>`:

```rust
use eyre::Result;
use vihaco::{component, Effects, Execute, Execution, StepResult};

component! {
    component Counter { value: i64, }
    instruction { Add(i64), Read, }
}

impl Execute<counter::instruction::Add> for counter::Counter {
    type Message = ();
    type Effect = ();
    type Fault = eyre::Report;
    fn execute(&mut self, instruction: &counter::instruction::Add, _: ()) -> Result<StepResult<()>> {
        self.value += instruction.0;
        Ok(StepResult { effects: Effects::none(), execution: Execution::Complete })
    }
}
```

## Workspace

vihaco is a Cargo workspace of focused crates — depend on what your workload
needs; there is no umbrella crate.

| Crate | Role |
|---|---|
| [`vihaco`](crates/vihaco) | The batteries-included facade: re-exports the instruction, message, effects, execution, component, and composite APIs, plus the module / syntax / runtime layers and `Value` / `Type` model. |
| [`vihaco-abi`](crates/vihaco-abi) | The ISA vocabulary: the `Instruction` / `Effects` types, the `Value` / `Type` model, and the encoding + host-VM traits. |
| [`vihaco-abi-derive`](crates/vihaco-abi-derive) | `#[derive(Instruction)]`, re-exported through `vihaco-abi`'s `derive` feature. |
| [`vihaco-bytecode`](crates/vihaco-bytecode) | The binary / SST container format: headers, sections, and instruction (de)coding. |
| [`vihaco-module`](crates/vihaco-module) | The loadable `Module` model, program loader, host-VM traits, and assembly-style `Display`. |
| [`vihaco-runtime`](crates/vihaco-runtime) | The component/machine runtime: `Execute<I>`, `StepResult`, `Execution`, `Supply`, `Absorb`, `Observe`, `Handle`, and effect machinery. |
| [`vihaco-runtime-derive`](crates/vihaco-runtime-derive) | The `component!` and `composite!` declaration macros, re-exported through `vihaco` and `vihaco-runtime`'s `derive` feature. |
| [`vihaco-stdlib`](crates/vihaco-stdlib) | Standard-library components and observers, including `StdoutObserver`. |
| [`vihaco-syntax`](crates/vihaco-syntax) | Typed SST parsing and module construction (`Resolve`). |
| [`vihaco-cpu`](crates/vihaco-cpu) | A ready-made CPU/host component — a small stack machine (constants, arithmetic, branches, halt, …). Use directly, or as a reference for writing your own. |
| [`vihaco-parser`](crates/vihaco-parser) | The `Parse<'src>` and `SurfaceInstruction` traits plus lexical, primitive, and collection implementations shared by the parser derive. |
| [`vihaco-parser-derive`](crates/vihaco-parser-derive) | `#[derive(Parse)]` — turns instruction, value, and type enums or structs into [chumsky](https://github.com/zesterer/chumsky) parsers via `#[syntax_class]` and `#[pattern]`. |

## Quick start

vihaco targets the **Rust 2024 edition** (rustc ≥ 1.85).

Add it as a dependency:

```toml
[dependencies]
vihaco = "0.1"
```

Until the first crates.io release is published, pin to the repository instead:
`vihaco = { git = "https://github.com/QuEraComputing/vihaco" }`.

To work **in** the repository, the toolchain and common tasks are managed with
[mise](https://mise.jdx.dev):

```bash
mise install      # rust, node (docs), prek, hawkeye
mise run setup    # install the git pre-commit hooks
mise run test     # cargo test --workspace --all-targets
```

No mise? A stable Rust 2024 toolchain is enough — `cargo test --workspace
--all-targets` and the usual `cargo fmt` / `cargo clippy` cover the rest. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the full task list.

## Documentation

Guides and the API reference are published to GitHub Pages:
**<https://queracomputing.github.io/vihaco/>**. The guides walk through defining
instructions, pattern parser integration, typed module resolution, messages,
components, observers, composites, and composite-owned program loading.

Every code block in the guides and on the site is compiled — and, where
runnable, executed — in CI (via the `vihaco-doctests` crate), so the examples
can't drift from the API. To preview the docs locally:

```bash
cd docs && pnpm install && pnpm dev
```

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for how to
build, test, and submit changes. By contributing you agree to the
[Contributor License Agreement](CLA.md).

## License

Licensed under the [MIT License](LICENSE). © The vihaco Authors — see
[AUTHORS](AUTHORS). Initially developed at QuEra Computing Inc.
