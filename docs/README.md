# vihaco docs site

Astro site for the vihaco documentation. Deployed to the `gh-pages`
branch by `.github/workflows/docs.yml`; PRs that touch this directory get
a per-PR preview deploy.

## Quick reference

From this directory:

```bash
npm install      # one-time
npm run dev      # astro dev → http://127.0.0.1:4321
npm run build    # astro build → ./dist/
npm run preview  # serve ./dist/ locally
```

## Layout

```
docs/
├── examples/                # *.rs snippets embedded in the landing hero (?raw)
├── src/
│   ├── layouts/
│   │   ├── Base.astro       # masthead, footer, theme toggle, copy buttons
│   │   └── Guide.astro      # markdown layout: sections nav + on-page TOC + pager
│   ├── pages/
│   │   ├── index.astro      # landing
│   │   ├── quickstart.astro # Rust quick start (the workspace crates)
│   │   ├── reference.astro  # pointer to the generated rustdoc
│   │   └── guide/*.md        # the guide tutorials (markdown)
│   ├── styles/global.css
│   └── guides.ts            # ordered guide list (drives nav + pager)
├── package.json
└── astro.config.mjs
```

## Notes

- There is no Python layer — vihaco is a pure-Rust workspace, so the site is
  Rust-only (no Python index, no notebook execution).
- Code blocks are highlighted client-side by highlight.js (themed with the
  page's design tokens), so Astro's build-time syntax highlighting is disabled
  in `astro.config.mjs`.
- Internal markdown links are written root-absolute (`/guide/components`) and
  prefixed with the deploy base by a small rehype plugin, so they survive both
  the gh-pages root and per-PR preview subpaths.

## The docs' code is tested

The code shown on the site is compiled (and, where runnable, executed) by the
`vihaco-doctests` crate so it can't drift from the API:

- `docs/examples/*.rs` — the exact files the landing/quick-start pages import
  via `?raw` — are `include!`d and compiled; the runnable ones run as `#[test]`s.
- Every fenced ` ```rust ` block in `docs/src/pages/guide/*.md` is a rustdoc
  doctest. Run them all with `cargo test --doc -p vihaco-doctests`. CI runs both
  `cargo test --workspace --all-targets` (examples) and `--doc` (guides).

Conventions for guide code blocks (standard rustdoc):

- A block compiles as-is by default. Add `# `-prefixed lines for setup that
  should compile but not appear on the page — a rehype plugin strips those
  lines (and un-escapes a leading `##` to `#`) before rendering.
- Mark a block ` ```rust ignore ` when it's a deliberate fragment (a trait
  reproduction, runtime pseudocode, or a snippet that references types defined
  outside the block), or ` ```rust no_run ` to compile without running.

## Prerequisites

- Node ≥ 20 (Astro requirement).
- A recent stable Rust toolchain (edition 2024) to run the doc tests.
