# Contributing to vihaco

Thanks for your interest in contributing. This document explains how
contributions are licensed and how to get a change merged.

## Licensing of contributions

By opening a pull request against this repository, **you agree that your
contribution is licensed under the [MIT License](LICENSE)
and that you accept the terms of the [vihaco Contributor License
Agreement](CLA.md)**, which grants QuEra Computing Inc. and downstream
recipients the rights described in that document.

You retain copyright on your contribution; the CLA grants licenses, it
does not assign ownership. See [`CLA.md`](CLA.md) for the full text. If
you are contributing on behalf of an employer, please make sure your
employer is aware of and authorizes the contribution — the CLA's
representations cover this in §4.

If you do not agree to those terms, please do not open a pull request.

## Where to start

- **Docs & guides** — <https://queracomputing.github.io/vihaco/> (or run
  `cd docs && npm install && npm run dev` to read them locally). Start with the
  quick start, then the guides: defining instructions, parser integration,
  messages, components, observers, and composites.
- **Set up the toolchain.** The repo uses [mise](https://mise.jdx.dev) to pin
  the Rust toolchain, Node (for the docs site),
  [prek](https://github.com/j178/prek) (pre-commit), and
  [hawkeye](https://github.com/korandoru/hawkeye) (license headers):

  ```bash
  mise install      # install the pinned tools
  mise run setup    # install the git pre-commit hooks (fmt, clippy, license header)
  ```

  Prefer not to use mise? A stable Rust 2024 toolchain (rustc ≥ 1.85) and
  `cargo` are enough for the commands below; Node ≥ 20 is only needed to work on
  the docs site.
- **Build & test** — each `mise run <task>` maps to the plain command shown, so
  you can run either:

  | Task | Command |
  |---|---|
  | `mise run test` | `cargo test --workspace --all-targets` |
  | `mise run doctest` | `cargo test --workspace --doc` — compiles the guide/example code |
  | `mise run fmt` | `cargo fmt --all` |
  | `mise run lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
  | `mise run license` | `hawkeye check` — SPDX headers |

- **License headers.** Every source file under `crates/**` carries a two-line
  SPDX header. Add one to a new file with `mise run license-fix` (or
  `hawkeye format`); CI enforces it. Line-sensitive trybuild fixtures under
  `tests/ui/` and `tests/compile_errors/` are intentionally excluded.

## Workflow

1. Fork the repository and create a topic branch off `main`.
2. Make your change. Keep commits focused and use
   [Conventional Commits](https://www.conventionalcommits.org/)
   (e.g. `feat(runtime): add new gate`).
3. Add or update tests for the behavior you changed.
4. Run `mise run test` / `mise run lint` (or the `cargo` equivalents) and the
   pre-commit hooks (`prek run --all-files`) locally.
5. Open a pull request. CI runs the project's tests and the license-header
   check.
6. A maintainer will review. Be prepared to iterate.

## Reporting issues

Use GitHub Issues for bugs and feature requests. For security-sensitive
reports, please email a maintainer privately rather than filing a public
issue: Kai-Hsin Wu <khwu@quera.com>, Neelay Fruitwala
<nfruitwala@quera.com>, or Roger Luo <me@rogerluo.dev>.
