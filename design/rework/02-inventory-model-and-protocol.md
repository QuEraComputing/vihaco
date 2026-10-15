# Step 2: inventory model and protocol

**PR:** [PR 1: shared inventories and the dialect producer](pull-request-plan.md#pr-1-shared-inventories-and-the-dialect-producer).

**Review/commit:** Complete this file's work and report it for user review. If
there are eligible changes outside `design/`, wait for explicit approval before
its dedicated commit. If the step changes only `design/`, record a no-commit
checkpoint and wait for review approval before starting the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contract records, probes, fixtures, and
results. Exclude them from every implementation commit and PR. Do not move
local planning artifacts elsewhere or create an empty commit merely to satisfy
a step's commit boundary.

**Source references:** `rob/inventory-test`; see [branch context](README.md#source-references-and-branch-context).

**Prerequisites:** [Step 1](01-contracts-and-baseline.md).
**Next:** [paths and names](03-paths-names-and-options.md).

## Outcome

Make inventory semantics independent of generated macro code. A validated
model should describe order, identities, route suffixes, and source steps;
one parser and renderer should own the wire grammar.

## Current implementation and lessons

[inventory/mod.rs](../../crates/vihaco-runtime-derive/src/inventory/mod.rs)
already has useful `Inventory`, `Entry`, `SourceStep`, and collection types.
Its parser rejects generic routes and checks fixed fields, but selectors are
still broad `syn::Path` values and identity uses token rendering. Consumers
can index malformed duplicate records without validating them first.

Preserve the simple schema; tighten its invariants at the shared boundary.
Do not return to the earlier alias-based protocol.

## Model and API

Use small internal types along these lines:

```text
Inventory { entries: Vec<Entry> }
Entry { selector: LogicalSelector, type_route: TypeRoute, source_steps: Vec<SourceStep> }
SourceStep { name: LitStr, aliases: Vec<LitStr> }
CollectedInventory { namespace: Path, tag: TokenStream2, inventory: Inventory }
CollectedInventories { records: Vec<CollectedInventory> }
```

`LogicalSelector` and `TypeRoute` wrap validated relative paths, retain `Ident`
spans, and expose segment iteration and prefix operations. Their structural
keys compare identifiers rather than whitespace in `TokenStream::to_string()`.
Normalize raw identifier spelling for identity, while preserving raw tokens
for emitted Rust paths. An authored qualified dialect path needs its own key
policy; it is not a logical selector.

Require plain relative segments without generics, qualified self, or a leading
root. Logical selectors contain a dialect and leaf suffix; type routes contain
one or more relative segments. Enforce the source-step relationship guaranteed
by producers: source steps correspond to the composite selector prefixes in
outermost-to-innermost order. If a protocol fixture intentionally omits that
relation, make it malformed rather than weakening the producer contract.

Use a `Vec` as the authoritative ordered storage. An auxiliary lookup can hold
indices or references; it must validate duplicates before insertion. Do not
make a map the primary inventory or derive execution identity from route indices.

## Implementation tasks

1. Extract model and transformations to `inventory/model.rs`. Keep `mod.rs` as
   a narrow set of internal exports.
2. Add constructors/validation for selectors, routes, and source steps. Validate
   source spellings according to Step 1’s recorded policy, including duplicate
   aliases and canonical-name collisions. Treat invalid wire input as
   `syn::Error`, never a proc-macro panic.
3. Implement shared ordered operations: concatenate complete inventories with
   full-selector uniqueness; prefix a composite field on both paths and its
   source step; prefix a provider on the type route alone; full-selector lookup
   with checked uniqueness.
4. Give the replacement shared API no leaf-name filtering operation. Components
   consume whole dialects; only instruction-set choices perform leaf selection,
   using full logical selectors even when leaf names coincide. Delete the draft's
   `select_names` alongside any existing component conversion in Steps 6–7;
   when starting from the inspected `main`, omit it entirely because its draft
   caller is absent. No replacement caller should depend on it.
   Consumers still validate the expected producer shape.
5. Extract protocol parsing/rendering to `inventory/protocol.rs`. Use one
   `Entry` renderer for all producers and one collected-record parser for all
   finalizers. Retain opaque tags and state tokens; parse consumer tags only
   in their own finalizer.
6. Add small braced-field/envelope helpers for the repeated finalizer grammar.
   Share syntax mechanics, not a universal context type with fields unused by
   some consumers. Check that every delimited body and outer invocation is
   exhausted.
7. Update users already present on the current stack base together. Build the
   reference wire shape directly where inventory support is new; introduce
   later macro consumers with their owning PRs rather than porting the entire
   draft into PR 1.

## Wire contract

The response remains:

```text
state: { opaque caller tokens },
inventory: {
    entry {
        selector: { core::cpu::Add },
        type_route: { core::provider_0::cpu::Add },
        source_steps: {
            step { name: "core", aliases: { "c", }, },
        },
    },
},
```

A collected record adds `namespace` and `tag`. Required fields, field order,
and comma rules are owned by the protocol module and documented there.
Keep the existing exporter request’s optional final comma. Internal response
rendering can keep mandatory commas; avoid permissiveness that makes malformed
records harder to diagnose.

Validate each received inventory before any consumer creates a map or selects
entries. This includes a malicious or hand-written provider whose duplicate
entries happen to have different type routes. The finalizer must reject that
input instead of choosing the last record.

## Verification

- Round-trip empty and populated inventories with nested source steps and
  aliases; assert semantic fields and order after parsing.
- Exercise full inventory concatenation in dialect-list/declaration order,
  full-selector lookup, duplicate selectors within one inventory and across several,
  provider route prefixes, and two successive composite prefixes.
- Reject missing/extra/repeated fields, trailing tokens, absolute paths,
  generic arguments, contextual root segments, malformed source steps, and
  inconsistent selector/source prefixes.
- Preserve useful spans on the later duplicate and authored unknown full selector.
- Keep the distinction between equal type routes and equal logical selectors:
  repeated leaf payloads through different fields remain valid.

Do not make unit tests depend on pretty-printer spacing or the number of
`quote!` fragments. Compiler/type identity checks belong in later integration
steps; this module does not try to resolve Rust aliases.

## Exit criterion

All consumers parse and transform one validated inventory model. Token grammar
appears only in its protocol module, and selector lookup cannot silently hide
duplicate exported entries. The existing focused suites still pass except for
deliberately improved malformed-input diagnostics reviewed in their fixtures.

## Implementation checkpoint

Step 2 was reviewed and approved. The implementation adds private `inventory/`
model, protocol, and tests to `vihaco-runtime-derive`, registers the module in
`lib.rs`, and enables test-only span locations on its existing `proc-macro2`
dependency. There are no inventory consumers on this stack base, so no public
macro entry points or diagnostic fixtures change in this step.

The model validates relative paths, normalizes raw identifiers for structural
identity, preserves authored tokens/spans, and checks full-selector duplicates
at construction and wire parsing. Ordered concatenation, provider-only route
prefixes, composite field/source prefixes, and checked full-selector lookup
operate on the validated model. Source steps must exactly match all selector
prefixes before the dialect/leaf suffix. Source spellings cannot designate
different sibling fields; repetition under different parents is permitted.

Source aliases are literal segments, not Rust identifiers. Empty spellings,
whitespace, control characters, `::`, duplicate aliases, and collisions with
canonical names are rejected; punctuation and Unicode remain supported.
Canonical source names use the unraw field spelling. Empty `<>` argument
syntax is rejected as well as nonempty generics.

One protocol module parses/renders entries and collected records, preserves
opaque state/tags, and shares braced-field/finalizer envelope mechanics.
Direct-response parsing is test-only; production finalizers will consume
collected records. The module has temporary private dead-code/re-export lint
allowances while its owning macros are introduced in later steps. No leaf-name
filter, exporter, collector, or placeholder finalizer was introduced.

`CollectedInventory` exposes its fields within the crate so consumers can borrow
or move each field independently. Its constructor still checks namespace syntax;
the record has no getters or consuming field accessor.

All 24 focused semantic tests passed, including diagnostic source positions.
Workspace format, Clippy with warnings denied, all-target tests, doctests, and
SPDX checks passed (89 files, zero conflicts). The preserved draft is unchanged;
its reference suites were established in Step 1.

Approved dedicated commit (`fbfec16`):
`refactor: introduce validated inventory model and protocol`.
Only the six changed/new files under `crates/vihaco-runtime-derive/` are eligible
for this commit; every `design/` file was excluded. The user approved the commit
and continuation to Step 3. No PR has been created.
