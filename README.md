# vihaco

A virtual ISA and machine framework for orchestrating hybrid analog/digital
quantum control. Define instruction sets, components, effects, and
pattern-derived source syntax as ordinary Rust — then compose them into a
machine.

[![CI](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml/badge.svg)](https://github.com/QuEraComputing/vihaco/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it is

Define instruction sets with `#[dialect]`, implement their behavior on
`#[component]` devices, and wire the devices together with `#[composite]`.
Components receive typed instructions and messages and return effects.
Pattern-derived parsers let programs be written in SST, the framework's
assembly-style source format.

The guide examples are compiled and tested in CI. [First machine](https://queracomputing.github.io/vihaco/guide/first-machine/) · [Program execution](https://queracomputing.github.io/vihaco/guide/program-execution/) · [Full documentation](https://queracomputing.github.io/vihaco/)

## Workspace

| Crate | Role |
|---|---|
| [`vihaco`](crates/vihaco) | Facade for declaring instructions and machines, parsing source, and running programs. |
| [`vihaco-runtime`](crates/vihaco-runtime) and [`vihaco-runtime-derive`](crates/vihaco-runtime-derive) | Component and composite contracts, declaration macros, dispatch, and effect observation. |
| [`vihaco-stdlib`](crates/vihaco-stdlib) | Program images, execution state, standard instructions, debugger, and observers. |
| [`vihaco-cpu`](crates/vihaco-cpu) | Standard computation and heap component. |
| [`vihaco-abi`](crates/vihaco-abi) and [`vihaco-abi-derive`](crates/vihaco-abi-derive) | Instruction encoding, values and types, effects, and the `Instruction` derive. |
| [`vihaco-bytecode`](crates/vihaco-bytecode) and [`vihaco-module`](crates/vihaco-module) | Bytecode and SST containers, loaded modules, and program metadata. |
| [`vihaco-parser`](crates/vihaco-parser), [`vihaco-parser-derive`](crates/vihaco-parser-derive), and [`vihaco-syntax`](crates/vihaco-syntax) | Pattern-derived parsing and printing, SST parsing, and lowering to runtime instructions. |
| [`vihaco-sst`](crates/vihaco-sst) | SST diagnostics, checking, formatting, and disassembly. |

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for how to
build, test, and submit changes. By contributing you agree to the
[Contributor License Agreement](CLA.md).

## License

Licensed under the [MIT License](LICENSE). © The vihaco Authors — see
[AUTHORS](AUTHORS). Initially developed at QuEra Computing Inc.
