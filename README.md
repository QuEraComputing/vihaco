# vihaco

A virtual ISA and machine framework for orchestrating hybrid analog/digital
quantum control. Define instruction sets, components, effects, and parsers as
ordinary Rust — then compose them into a machine.

[![CI](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml/badge.svg)](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it is

vihaco is a framework for building small virtual machines. You define

- the **instruction set** — an enum, with `#[derive(Instruction)]`;
- the **components** that execute it — with `#[component]`;
- the **effects** they emit; and
- (optionally) a **parser** for SST — with `#[derive(Parse)]`,

all as ordinary Rust, then compose them into a machine. A component is one
`execute(instruction, message) -> effects`:

```rust
use eyre::Result;
use vihaco::{Effects, Instruction, Message, component};

// Bytecode-visible operations: each variant is an opcode, tuple fields its payload.
#[derive(Debug, Clone, Instruction)]
pub enum CounterInst {
    Add(i64),
    Print,
}

// Runtime-supplied input, not encoded in the instruction stream.
#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

// A value the component emits for the runtime / observers to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[derive(Debug, Default)]
pub struct Counter {
    value: i64,
}

#[component(instruction = CounterInst, message = Prefix, effect = Line)]
impl Counter {
    fn execute(&mut self, inst: CounterInst, msg: Prefix) -> Result<Effects<Line>> {
        match inst {
            CounterInst::Add(v) => {
                self.value += v;
                Ok(Effects::none())
            }
            CounterInst::Print => Ok(Effects::one(Line(format!("{}{}", msg.0, self.value)))),
        }
    }
}
```

## Workspace

vihaco is a Cargo workspace of focused crates — depend on what your workload
needs; there is no umbrella crate.

| Crate | Role |
|---|---|
| [`vihaco`](crates/vihaco) | The framework: the `Instruction` / `Message` / `Effects` types and their derives, the `#[component]` / `#[observe]` / `#[composite]` macros, the module / syntax / runtime layers, and the `Value` / `Type` model. Re-exports the macros, so most projects depend only on this crate. |
| [`vihaco-cpu`](crates/vihaco-cpu) | A ready-made CPU/host component — a small stack machine (constants, arithmetic, branches, halt, …) with a `StepOutcome` control-flow effect. Use directly, or as a reference for writing your own. |
| [`vihaco-parser`](crates/vihaco-parser) | `#[derive(Parse)]` — turns an instruction enum into a [chumsky](https://github.com/zesterer/chumsky) parser via `#[head]` / `#[token]` / `#[delimiters]` / `#[parse_with]` attributes. |
| [`vihaco-parser-core`](crates/vihaco-parser-core) | The `Parse<'src>` trait and primitive impls shared by the parser derive. |
| [`vihaco-derive`](crates/vihaco-derive) | The procedural macros behind the derives (used via `vihaco`'s re-exports). |

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
instructions, parser integration, messages, components, observers, and
composites.

Every code block in the guides and on the site is compiled — and, where
runnable, executed — in CI (via the `vihaco-doctests` crate), so the examples
can't drift from the API. To preview the docs locally:

```bash
cd docs && npm install && npm run dev
```

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for how to
build, test, and submit changes. By contributing you agree to the
[Contributor License Agreement](CLA.md).

## License

Licensed under the [MIT License](LICENSE). © The vihaco Authors — see
[AUTHORS](AUTHORS). Initially developed at QuEra Computing Inc.
