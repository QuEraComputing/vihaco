# Releasing vihaco

Releases are automated with [release-plz](https://release-plz.dev) and driven by
[Conventional Commits](https://www.conventionalcommits.org/). In normal
operation you never run `cargo publish` by hand — you merge a PR.

## The normal flow

1. Land feature/fix PRs to `main` with conventional-commit messages
   (`feat(runtime): …`, `fix(parser): …`, etc.).
2. The **`release-plz-pr`** job opens (and keeps updating) a single **Release
   PR** titled like `chore(release): 0.2.0`. It bumps the workspace version and
   regenerates each crate's `CHANGELOG.md` from the commits since the last
   release.
3. Review that PR. When it looks right, **merge it**.
4. Merging is a push to `main`, which triggers the **`release-plz-release`**
   job: it tags the release, creates the GitHub Release(s), and publishes the
   changed crates to crates.io.

That's it. To cut a release, you merge the Release PR.

> **The Release PR does not run CI.** It's opened with the default
> `GITHUB_TOKEN`, and GitHub does not start new workflow runs for events raised
> by that token. This is expected: the Release PR only bumps versions and
> changelogs, and the feature PRs that fed it already passed CI. (To get CI on
> the Release PR, swap the token for a PAT or GitHub App — see *Auth* below.)

## Versioning

The crates share **one version** (`version.workspace = true` in the root
`Cargo.toml`), so the whole workspace is released **in lockstep** under a single
version number.

While the project is pre-1.0, release-plz applies Cargo's 0.x semver rules from
the commit types:

| Commits since last release | Bump | Example |
|---|---|---|
| `fix:` / `feat:` | patch | `0.1.0 → 0.1.1` |
| `feat!:` or a `BREAKING CHANGE:` footer | minor | `0.1.0 → 0.2.0` |

Publishable crates (released together):

- `vihaco`, `vihaco-cpu`, `vihaco-derive`, `vihaco-parser`, `vihaco-parser-core`

`vihaco-doctests` is `publish = false`, so release-plz skips it automatically.

## Auth

| Target | Mechanism | Notes |
|---|---|---|
| GitHub | default `GITHUB_TOKEN` | No PAT/App, so no org-admin setup. Tradeoff: no CI on the Release PR (above). |
| crates.io | **OIDC trusted publishing** | No stored token. `id-token: write` on the release job lets release-plz mint a short-lived token. |

crates.io trusted publishing is configured **per crate** (you must be a crate
owner — no GitHub org access needed): for each crate, go to
`https://crates.io/crates/<name>/settings` → **Trusted Publishing** → **GitHub**
and set owner `QuEraComputing`, repo `vihaco`, workflow `release-plz.yml`,
environment *(blank)*.

Each job runs with least-privilege permissions: the **PR job** uses
`contents: write` + `pull-requests: write`; the **release job** uses
`contents: write` + `pull-requests: read` + `id-token: write`. The
`pull-requests: read` is required — release-plz lists the PRs behind the release
commit for the notes, and without it the release fails with
`403 "Resource not accessible by integration"`.

## Adding a new crate to the workspace

Trusted publishing can only be configured for a crate that **already exists** on
crates.io, so a brand-new crate needs a one-time manual bootstrap:

1. Add the crate under `crates/`. Give it a `description`; it inherits
   `version`/`license`/`repository`/`authors` from `[workspace.package]`. If it
   should never be published (dev/test only), set `publish = false`.
2. **Publish it once by hand** (logged in via `cargo login`):

   ```bash
   cargo publish --manifest-path crates/<name>/Cargo.toml
   ```

   If other new crates depend on it, publish in dependency order (see *Manual
   release* below).
3. Configure its trusted publisher on crates.io (see *Auth*).

After that, release-plz publishes it automatically with the rest.

## Gotchas

### `release-plz/action` is pinned by a version tag

The action is pinned by tag, matching release-plz's docs and the other
version-pinned actions in this repo:

```yaml
uses: release-plz/action@<tag>   # e.g. v0.5.130
```

**Heads-up for automated edits:** a `name@version` ref looks like an email
address, so editing this file through a tool that applies email obfuscation
(some web proxies, some AI assistants) can rewrite the ref into an obfuscated
placeholder. The result is an invalid `uses:` value, and GitHub then rejects the
**entire workflow at startup** — runs show 0 jobs and no logs, and the workflow
name falls back to the file path. If you edit the workflow with such a tool,
re-check the `release-plz/action` lines afterward.

Bump the version by changing the tag (or let Dependabot do it).

### Fixing a bad release

Crates can't be deleted, only **yanked** (reversible):

```bash
cargo yank --version X.Y.Z <crate>          # hide from new resolutions
cargo yank --version X.Y.Z <crate> --undo   # reverse it
```

Note that yanking a crate breaks any **already-published** crate that depends on
that exact version (fresh installs can't resolve it). The fix is to ship a new
patch release that repoints the dependents — exactly what the normal flow does.

## Manual release (fallback)

If you ever need to release without the workflow, bump the version in the root
`Cargo.toml` (`[workspace.package]` and the `[workspace.dependencies]`
requirements), then publish in dependency order — each crate must be on the
index before the crates that depend on it:

```text
vihaco-parser-core → vihaco-derive → vihaco-parser → vihaco → vihaco-cpu
```

```bash
cargo publish --manifest-path crates/<name>/Cargo.toml
```

Recent `cargo` waits for each crate to appear in the index before returning, so
the next publish in the chain resolves cleanly.
