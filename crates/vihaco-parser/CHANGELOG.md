# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-parser-v0.2.0...vihaco-parser-v0.3.0) - 2026-08-14

### Other

- [**breaking**] split vihaco monolith into focused crates (+ per-trait derive crates) ([#50](https://github.com/QuEraComputing/vihaco/pull/50))
- Updated and fixed syntax/parser implementation for missing derives, missing generic type parsing, and automatic `SurfaceInstruction` impl ([#48](https://github.com/QuEraComputing/vihaco/pull/48))

## [0.2.0](https://github.com/QuEraComputing/vihaco/compare/vihaco-parser-core-v0.1.1...vihaco-parser-core-v0.2.0) - 2026-07-29

### Other

- Parser refactor to support values, types, instructions ([#40](https://github.com/QuEraComputing/vihaco/pull/40))
- Refactor multi-section implementation ([#37](https://github.com/QuEraComputing/vihaco/pull/37))
