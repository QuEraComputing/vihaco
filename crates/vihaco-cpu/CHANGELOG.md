# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-cpu-v0.2.0...vihaco-cpu-v0.3.0) - 2026-08-14

### Other

- Added `component!` macro ([#57](https://github.com/QuEraComputing/vihaco/pull/57))
- [**breaking**] split vihaco monolith into focused crates (+ per-trait derive crates) ([#50](https://github.com/QuEraComputing/vihaco/pull/50))
- Updated and fixed syntax/parser implementation for missing derives, missing generic type parsing, and automatic `SurfaceInstruction` impl ([#48](https://github.com/QuEraComputing/vihaco/pull/48))

## [0.2.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-cpu-v0.1.1...vihaco-cpu-v0.2.0) - 2026-07-29

### Other

- Parser refactor to support values, types, instructions ([#40](https://github.com/QuEraComputing/vihaco/pull/40))
- Refactor multi-section implementation ([#37](https://github.com/QuEraComputing/vihaco/pull/37))
- Multi-section bytecode support ([#31](https://github.com/QuEraComputing/vihaco/pull/31))

## [0.1.1](https://github.com/QuEraComputing/vihaco/releases/tag/vihaco-cpu-v0.1.1) - 2026-06-22

### Other

- rename vihaco-macros crate to vihaco-derive ([#21](https://github.com/QuEraComputing/vihaco/pull/21))
- open-source setup — MIT legal files, SPDX headers, license CI + mise/prek ([#19](https://github.com/QuEraComputing/vihaco/pull/19))
- Add HeapDealloc ([#11](https://github.com/QuEraComputing/vihaco/pull/11))
- Fix function returns in vihaco-cpu ([#3](https://github.com/QuEraComputing/vihaco/pull/3))
- Prep crates: workspace metadata + internal dep restructure ([#2](https://github.com/QuEraComputing/vihaco/pull/2))
- Import vihaco crates and set up workspace + CI
