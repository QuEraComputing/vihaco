# vihaco

A virtual ISA and machine framework for orchestrating hybrid analog/digital
quantum control. Define instruction sets, components, effects, and
pattern-derived source syntax as ordinary Rust — then compose them into a
machine.

[![CI](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml/badge.svg)](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it is

vihaco is a framework for building small virtual machines. You define

- the **instruction set** — usually inside a `component!` declaration (or a
  standalone enum with `#[derive(Instruction)]`);
- the **components** that execute it — with `component!`;
- the **effects** they emit; and
- (optionally) **SST source syntax** — with `#[derive(Parse)]`,

all as ordinary Rust, then compose them into a machine. A component is one
`execute(instruction, message) -> effects`:

```rust
use eyre::Result;
use vihaco::{component, dispatch, Effects, Message};

component! {
    #[derive(Debug, Default)]
    pub component Counter {
        value: i64,
    }

    instruction {
        Add(i64),
        Print,
    }
}

use counter::runtime::Instruction as CounterInst;

// Runtime-supplied input, not encoded in the instruction stream.
#[derive(Debug, Clone, Message)]
pub struct Prefix(pub String);

// A value the component emits for the runtime / observers to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Line(pub String);

#[dispatch(instruction = counter::runtime::Instruction, message = Prefix, effect = Line)]
impl counter::Counter {
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

## Experimental typed declarations

The nondefault `typed-declarations` feature enables `#[dialect]`,
`#[component]`, and typed `#[composite]`. Components declare complete dialects
and implement `Execute<I>` for every instruction. Composites generate flat
runtime instruction enums, direct parsers, and `Dispatch`; provide
`Step<field!(Machine::destination), Instruction>` for every destination/leaf pair.
Device codes are explicit u8 values, unique within each parent. Ordinary state
and loadable subtrees do not contribute parent instructions.

Parsing produces runtime instructions directly, with no paired surface enum or
separate `Resolve` pass. Composite source uses `device::nested_device::dialect::mnemonic`
prefixes and accepts device aliases. Use `HasInstructionSet::Runtime` to access
the aggregate type. Renamed child imports need matching generated inventory and
field companion reexports; named generic children are supported, while bare
generic or associated child types cannot supply an inferred inventory.

Facade applications enable **`vihaco/typed-declarations`**, which forwards through
stdlib to CPU and runtime. Direct CPU consumers enable
**`vihaco-cpu/typed-declarations`**. Its `cpu` module
exposes instruction structs whose phase defaults to runtime. Constants parse
readable syntax literals and lower them into runtime words. Branches and calls
accept numeric addresses or `@name` targets; function-reference constants accept
indices or function symbols. String constants accept indices or quoted literals.
Pattern bindings infer
their field parser by default; `$0:SyntaxType` retains the annotated syntax type
and requires explicit lowering when it differs from the runtime field. Encoded
constants use literal newtypes to preserve their bits. `label @name` records its
position in the parsed function's label metadata and emits no instruction.
`function_start` and `function_end` are removed from
the typed dialect. Instructions use `()` or the `message::FunctionInfo` and
`message::Print` structs, returning `StepOutcome` through `eyre::Result`.

CPU lowering lives outside the dialect definitions in `vihaco_cpu::lower`.
Loaders implement `LabelLookup` for function-local labels and `FunctionLookup`
for section function addresses and indices. `StringIntern` supplies string-table
indices; `vihaco_stdlib::ProgramImage` implements it with deduplication of decoded contents.
Prepare functions and label positions before calling
`LoadOwnSstSection::lower_instructions` so forward references resolve. The
generated composite aggregate enforces these capabilities even for programs
using only numeric operands. Heap-reference constants continue to accept numeric IDs.

Generated hierarchical bytecode codecs remain separate work. Cargo feature
unification can activate the experimental contract indirectly. Runtime-only
feature activation cannot select CPU's local cfg; use the facade or CPU feature
for graphs containing a typed CPU. CPU features only activate lower crates. Legacy
`component!` examples and guide snippets retain a separate feature-off graph.

`ProgramImage` now lives in `vihaco-stdlib`; `vihaco::ProgramImage` and
`vihaco::loader::ProgramImage` remain available. Direct users of
`vihaco_module::loader::ProgramImage` or `vihaco_runtime::loader::ProgramImage`
should import `vihaco_stdlib::ProgramImage` instead. Generic loader and host
traits remain in `vihaco-module`. Direct stdlib consumers enable
`vihaco-stdlib/typed-declarations` for CPU string interning; this forwards to CPU
and runtime. Enabling only a CPU feature does not activate stdlib above it.

## Workspace

vihaco is a Cargo workspace of focused crates — depend on what your workload
needs; there is no umbrella crate.

| Crate | Role |
|---|---|
| [`vihaco`](crates/vihaco) | The batteries-included facade: re-exports every crate below at stable paths (`Message` / `Effects`, the `component!` / `#[observe]` / `#[composite]` macros, the module / syntax / runtime layers, the `Value` / `Type` model), so most projects depend only on this crate. |
| [`vihaco-abi`](crates/vihaco-abi) | The ISA vocabulary: the `Instruction` / `Effects` types, the `Value` / `Type` model, and the encoding + host-VM traits. |
| [`vihaco-abi-derive`](crates/vihaco-abi-derive) | `#[derive(Instruction)]`, re-exported through `vihaco-abi`'s `derive` feature. |
| [`vihaco-bytecode`](crates/vihaco-bytecode) | The binary / SST container format: headers, sections, and instruction (de)coding. |
| [`vihaco-module`](crates/vihaco-module) | The loadable `Module` model, program loader, host-VM traits, and assembly-style `Display`. |
| [`vihaco-runtime`](crates/vihaco-runtime) | The component/machine runtime: `GeneratedComponent`, effect sinks, and observation machinery. |
| [`vihaco-runtime-derive`](crates/vihaco-runtime-derive) | `#[derive(Message)]`, `component!`, `#[composite]`, `#[observe]`, re-exported through `vihaco-runtime`'s `derive` feature. |
| [`vihaco-stdlib`](crates/vihaco-stdlib) | Standard-library components and observers, including `ProgramImage` and `StdoutObserver`. |
| [`vihaco-syntax`](crates/vihaco-syntax) | Typed SST parsing and module construction (`Resolve`). |
| [`vihaco-cpu`](crates/vihaco-cpu) | A ready-made CPU/host component — a small stack machine (constants, arithmetic, branches, halt, …) with a `StepOutcome` control-flow effect. Use directly, or as a reference for writing your own. |
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

The nondefault `typed-declarations` feature enables typed declarations and
execution contracts in `vihaco` or `vihaco-runtime`, including derive support.
In this mode, `Component` and `Composite` no longer inherit `HasInstructionSet`;
generic code needing the runtime aggregate states that bound explicitly.
Capability reporting remains under `__private`. The default API remains
available while the typed declarations and direct parsing stabilize.

Guides and the API reference are published to GitHub Pages:
**<https://queracomputing.github.io/vihaco/>**. The guides walk through defining
instructions, pattern parser integration, messages, components, observers, and
composites.

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
