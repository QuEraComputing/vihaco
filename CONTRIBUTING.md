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

<!-- TODO: project-specific pointers. Examples:
  - "Code overview": link to the canonical developer guide.
  - "Build & test": exact commands (`cargo test --workspace`,
    `uv run pytest`, `npm run build`, etc.).
  - "Style": link to formatter / lint commands. Mention the
    license-header check if you've enabled it. -->

## Workflow

1. Fork the repository and create a topic branch off `main`.
2. Make your change. Keep commits focused and use
   [Conventional Commits](https://www.conventionalcommits.org/)
   (e.g. `feat(runtime): add new gate`).
3. Add or update tests for the behavior you changed.
4. Run the relevant test command(s) and any pre-commit checks locally.
5. Open a pull request. CI runs the project's tests and license-header
   check.
6. A maintainer will review. Be prepared to iterate.

## Reporting issues

Use GitHub Issues for bugs and feature requests. For security-sensitive
reports, please email a maintainer privately rather than filing a public
issue: Kai-Hsin Wu <khwu@quera.com>, Neelay Fruitwala
<nfruitwala@quera.com>, or Roger Luo <me@rogerluo.dev>.
