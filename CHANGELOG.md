# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!--
This is the ONE changelog for the whole workspace. Every crate shares a single
version and ships under a single `v0.x.y` tag, so per-crate changelogs would
each hold a fragment of the same story; the entries below are the union of the
twelve files this replaces.

release-plz maintains it via the `vihaco` package's `changelog_path` +
`changelog_include` (see release-plz.toml). It inserts new releases directly
below the `## [Unreleased]` heading, so do not remove or reword that line.
-->

## [Unreleased]

## [0.4.0](https://github.com/QuEraComputing/vihaco/compare/v0.3.1...v0.4.0) - 2026-08-19

### Other

- Allow nested composites with `#[composite]` macro ([#78](https://github.com/QuEraComputing/vihaco/pull/78))

## [0.3.1](https://github.com/QuEraComputing/vihaco/compare/v0.3.0...v0.3.1) - 2026-08-19

### Other

- *(release)* one `v0.x.y` tag and GitHub Release per release ([#71](https://github.com/QuEraComputing/vihaco/pull/71))
- Make `#[syntax_class(..)]` `head` argument optional ([#66](https://github.com/QuEraComputing/vihaco/pull/66))

## [0.3.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-v0.2.0...vihaco-v0.3.0) - 2026-08-18

### Other

- Added parsing for list of `name: type` parameters in function ([#65](https://github.com/QuEraComputing/vihaco/pull/65))
- Added `GlobalClock` component to stdlib crate ([#63](https://github.com/QuEraComputing/vihaco/pull/63))
- Allow new `metadata` class for pattern parser ([#59](https://github.com/QuEraComputing/vihaco/pull/59))
- Added `component!` macro ([#57](https://github.com/QuEraComputing/vihaco/pull/57))
- [**breaking**] split vihaco monolith into focused crates (+ per-trait derive crates) ([#50](https://github.com/QuEraComputing/vihaco/pull/50))
- Updated and fixed syntax/parser implementation for missing derives, missing generic type parsing, and automatic `SurfaceInstruction` impl ([#48](https://github.com/QuEraComputing/vihaco/pull/48))

<!--
#65 and #63 are documented here after the fact. The 0.3.0 release PR generated
its changelogs at `6a8e24c`, but the release job did not run until four days
later, tagging and publishing from `2d60335` — so both PRs are inside the
published 0.3.0 crates while appearing in none of the per-crate changelogs.
-->

## [0.2.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-v0.1.1...vihaco-v0.2.0) - 2026-07-29

### Other

- Parser refactor to support values, types, instructions ([#40](https://github.com/QuEraComputing/vihaco/pull/40))
- Refactor multi-section implementation ([#37](https://github.com/QuEraComputing/vihaco/pull/37))
- Multi-section bytecode support ([#31](https://github.com/QuEraComputing/vihaco/pull/31))

## [0.1.1](https://github.com/QuEraComputing/vihaco/releases/tag/vihaco-v0.1.1) - 2026-06-22

### Other

- rename vihaco-macros crate to vihaco-derive ([#21](https://github.com/QuEraComputing/vihaco/pull/21))
- open-source setup — MIT legal files, SPDX headers, license CI + mise/prek ([#19](https://github.com/QuEraComputing/vihaco/pull/19))
- Cleanup derive_machine.rs ([#14](https://github.com/QuEraComputing/vihaco/pull/14))
- Add HeapDealloc ([#11](https://github.com/QuEraComputing/vihaco/pull/11))
- Fix function returns in vihaco-cpu ([#3](https://github.com/QuEraComputing/vihaco/pull/3))
- Prep crates: workspace metadata + internal dep restructure ([#2](https://github.com/QuEraComputing/vihaco/pull/2))
- Import vihaco crates and set up workspace + CI
