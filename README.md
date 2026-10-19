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
and loadable subtrees do not contribute parent instructions. Generated
`Field<Route>::get` and `get_mut` borrow direct or nested devices, including
devices containing borrowed data.

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
`vihaco-stdlib/typed-declarations` for CPU string interning, or
`vihaco-stdlib/program-image-execution` for the execution protocols; these forward
to CPU and runtime. Enabling only a CPU feature does not activate stdlib above it.

The nondefault `program-image-execution` feature on `vihaco-stdlib`, `vihaco-cpu`,
`vihaco`, or `vihaco-runtime` implies `typed-declarations`. The host owns a
`LocalModule`; the machine contains an `ExecutionState` component marked
`#[program]`, which supplies execution-state access and operand-storage wiring.
Call `vihaco_stdlib::execution_state::execute(&module, &mut machine, arguments)`
(or the facade's `execute`) to enter main. `resume(&module, &mut machine)` requires
the same unchanged module associated with that invocation; identity is not enforced.

`ExecutionState` owns call frames, locals, operands, private PC, and halt/running
flags. It stores no module, Arc, loader context, or instruction type parameter.
`start(&module, arguments)` creates main's frame; `reset()` clears invocation
state while retaining allocated storage. `stack()`, `frames()`, and `pc()` provide
read-only inspection. The legacy `ProgramImage` remains a separate loader/host API.

The runner captures a code reference before its loop. Instructions borrow that
external module while Dispatch mutably borrows the machine, requiring no instruction
clone or Arc operation. Dispatch, authored Step, and BuildMessage forward a shared
execution context, here `&LocalModule`. Execute and Apply remain unchanged.
The runner establishes sequential continuation before dispatch; execution-state
instructions replace it for branches, calls, and returns. Composite authors never
manage the PC. Halt/root return and errors retain the source instruction position.
Earlier consumption and component mutations are not rolled back.

Component authors select `PopFromStack` messages and `PushToStack` effects; standard
builders transfer operands and resolve call/branch metadata. Authors implement Steps
and choose effect handling; no Step or Apply policy is generated. Custom message
builders remain available. Tuple inputs/outputs support arities 0–16 and homogeneous
array outputs support any length. Boolean extraction treats any nonzero word as true
and produces canonical `0`/`1` results.

`StackValue::Storage` separates public computation types from their internal storage
representation. Components receive typed values; storage can change without changing
their message/effect shapes. `PopStack`, `PushStack`, `StackValue`, `HasOperandStack`,
and `Apply` are framework extension contracts, not stack logic ordinary authors
must implement. Custom value types provide storage conversions.

Constants, locals, stack and control instructions live in
`vihaco_stdlib::program::program`, with parsing/lowering helpers in `program::lower`
and `program::operand`. Module construction and string interning happen before
execution. CPU computes with typed operands and returns effects; it owns no stack,
frames, or PC. Its heap exchanges operands through the same stack protocols.
HeapAlloc and HeapPush consume available operands sequentially on underflow.
Print belongs to a separate component; no Span or Breakpoint handling is added.
Feature-disabled legacy and typed CPU behavior is retained.

For variable input counts, use `PopFromStack<StackBatch<T>>` and implement
`StackInputCount<I>` on the component. `InputCount::Immediate(n)` obtains the count
from the instruction; `FromStack` consumes a word count above the values. Batches
preserve original stack order and own their contents. Up to eight values stay
inline; nine or more use heap storage. Components can retain a batch and use
slices or iteration without managing storage. Mixed tuple/batch messages and
dynamic batch outputs are not supplied.

Heap state survives program restart/reset; explicitly reset CPU to clear its heap.
Word encodings are shared in `vihaco-abi::word` and re-exported unchanged through
`vihaco_cpu::word` and the CPU crate root.

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

The legacy guide examples run in the default feature configuration. CI checks
typed declarations and borrowed-program execution in separate runs, excluding the
legacy `vihaco-doctests` crate so feature unification does not switch those examples
to a different API. The runnable example in
[`ExecutionState`'s documentation](crates/vihaco-stdlib/src/execution_state.rs)
exercises an externally owned module, authored Steps, and execution.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for how to
build, test, and submit changes. By contributing you agree to the
[Contributor License Agreement](CLA.md).

## License

Licensed under the [MIT License](LICENSE). © The vihaco Authors — see
[AUTHORS](AUTHORS). Initially developed at QuEra Computing Inc.
