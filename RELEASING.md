# Releasing vihaco

Releases are automated with [release-plz](https://release-plz.dev) and driven by
[Conventional Commits](https://www.conventionalcommits.org/). In normal
operation you never run `cargo publish` by hand — you merge a PR.

## The normal flow

1. Land feature/fix PRs to `main` with conventional-commit messages
   (`feat(runtime): …`, `fix(parser): …`, etc.).
2. The **`release-plz-pr`** job opens (and keeps updating) a single **Release
   PR** titled like `chore(release): 0.2.0`. It bumps the workspace version and
   regenerates the root `CHANGELOG.md` from the commits since the last release.
3. Review that PR. When it looks right, **merge it**.
4. Merging is a push to `main`, which triggers the **`release-plz-release`**
   job: it publishes the crates to crates.io, then creates **one** `v0.x.y` tag
   and **one** GitHub Release.

That's it. To cut a release, you merge the Release PR.

To see what the next release would look like before any of this, run
`mise run release-preview`. It rewrites `Cargo.toml` and `CHANGELOG.md` in place
without touching the remote — inspect with `git diff`, then `git checkout .`.

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

- `vihaco`, `vihaco-abi`, `vihaco-abi-derive`, `vihaco-bytecode`, `vihaco-cpu`,
  `vihaco-module`, `vihaco-parser`, `vihaco-parser-derive`, `vihaco-runtime`,
  `vihaco-runtime-derive`, `vihaco-stdlib`, `vihaco-syntax`

`vihaco-doctests` is `publish = false`, so release-plz skips it automatically.

### `version_group` keeps them in lockstep

`version.workspace = true` is not by itself enough to make release-plz treat the
workspace as one unit: it infers a bump **per crate** and would rewrite that one
shared field on behalf of whichever crate happened to change. So every published
crate is pinned into `version_group = "workspace"` in `release-plz.toml`. Each
member then gets the highest bump any member warrants, and they all release
together.

That is also what keeps the single tag working — see below.

## One tag, one release, one changelog

release-plz's default naming is per-package, and 0.3.0 shipped that way: twelve
tags and twelve GitHub Releases for one workspace version. Because GitHub's
"Latest" badge is per-*repository*, it landed arbitrarily on
`vihaco-abi-derive-v0.3.0`. `release-plz.toml` now collapses that into a single
`v0.x.y`:

| Key | Where | Why |
|---|---|---|
| `git_tag_name = "v{{ version }}"` | `[workspace]` | Default is `{{ package }}-v{{ version }}`. |
| `git_release_name = "v{{ version }}"` | `[workspace]` | A **separate** key — it does *not* inherit from `git_tag_name`. Left at its default, the tag would read `v0.4.0` but the Release title would still read `vihaco-v0.4.0`. |
| `git_tag_enable = false`, `git_release_enable = false` | `[workspace]` | With the name templated above, twelve packages would each try to create the identical tag and collide. Creation is re-enabled on `vihaco` only. |
| `changelog_update = false` | `[workspace]` | Re-enabled on `vihaco` only, so exactly one changelog is written. |
| `changelog_path = "CHANGELOG.md"` | `[[package]] vihaco` | One changelog at the repo root. Relative to the root `Cargo.toml`, and **cannot** be set in `[workspace]` — it is a package-only field. |
| `changelog_include = [...]` | `[[package]] vihaco` | Makes the release body *complete*. See below. |

**`vihaco` is the tag owner.** It is also 11th of 12 in publish order, so the tag
lands after nearly every crates.io publish has already succeeded.

### Why `changelog_include` is not optional

release-plz attributes a commit to a package by **which package directory the
changed files live in**. So `vihaco`'s changelog would otherwise contain only
commits touching `crates/vihaco/` — a `fix(parser):` confined to
`crates/vihaco-parser/` would be missing from the release notes entirely, with
nothing failing. `changelog_include` lists the other eleven published crates so
their commits land in the one changelog too.

Commits that touch **no** package directory (`docs/`, `README.md`, the root
`Cargo.toml`, CI) belong to no package and appear in no changelog. That is
release-plz's model, not a misconfiguration.

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

   **If it is published, add it to `release-plz.toml` in two places** — neither
   has a `[workspace]`-level form, and neither failure is loud:

   - its own `[[package]]` entry with `version_group = "workspace"`, or it
     silently versions on its own, out of step with the rest;
   - `vihaco`'s `changelog_include` list, or its commits silently vanish from
     every future release note.

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

### The tag owner must be a crate release-plz actually processes

release-plz only processes crates it publishes. If `git_tag_enable` is ever moved
to a crate with `publish = false` — or to one that a given release does not touch
— **no tag and no GitHub Release are created, and nothing fails to tell you so.**
The run is green and the release simply has no tag.

`version_group = "workspace"` is what makes `vihaco` safe as the owner: every
published crate releases together, so `vihaco` is always in the released set.
Removing `version_group` from the crates re-opens this hole.

### Tags before v0.4.0 look different

Releases up to and including 0.3.0 used per-crate tags (`vihaco-v0.3.0`,
`vihaco-parser-v0.3.0`, …); those 22 tags and their GitHub Releases are left in
place as history. An annotated `v0.3.0` tag was added at the same commit the
0.3.0 tags point to (`2d60335`), so release-plz can resolve the previous version
under the new pattern and start the v0.4.0 changelog from the right place. It has
no GitHub Release attached.

Note that `git_only` is left at its default `false`, so release-plz determines
the previous version from **crates.io**, not from tags. The tags only bound the
changelog commit range.

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
vihaco-abi-derive → vihaco-abi → vihaco-bytecode → vihaco-module →
vihaco-runtime-derive → vihaco-runtime → vihaco-parser → vihaco-parser-derive →
vihaco-stdlib → vihaco-syntax → vihaco → vihaco-cpu
```

```bash
cargo publish --manifest-path crates/<name>/Cargo.toml
```

Recent `cargo` waits for each crate to appear in the index before returning, so
the next publish in the chain resolves cleanly.

Then tag and write the notes by hand, matching what the automation would have
done — one tag for the whole workspace:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
gh release create vX.Y.Z --title vX.Y.Z --notes-file <(…)   # notes from CHANGELOG.md
```
