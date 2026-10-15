# Step 5: reimplement `dialect!`

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

**Prerequisites:** [Steps 1–4](README.md#table-of-contents-and-implementation-order).
**Next:** [component first expansion](06-component-input-and-first-expansion.md).

## Outcome

Generate each dialect’s canonical runtime structs, corresponding surface
structs, leaf parsers, and full declaration-ordered inventory from one validated
declaration. The dialect remains a passive leaf provider.

Each component using this dialect must execute its entire inventory. A new leaf
therefore adds a required coverage obligation to every component declaring the
dialect; the exporter always includes it without an opt-in selection mechanism.

## Starting points

The current [parser](../../crates/vihaco-runtime-derive/src/dialect/parse.rs)
already handles unit and tuple declarations and `Syntax => Runtime` mappings.
[validation.rs](../../crates/vihaco-runtime-derive/src/dialect/validation.rs)
checks nonempty declarations, attributes, names, and mnemonic uniqueness.
[mod.rs](../../crates/vihaco-runtime-derive/src/dialect/mod.rs) mixes leaf
generation, tree layout, option handling, and inventory construction.

Keep the parser’s compact grammar and the current public leaf API. Use the
new shared support for everything concerning paths, inventory data, and export.

## Declaration model and ownership

Use `Declaration { attrs, name, instructions }` and
`Instruction { attrs, name, fields }`, with each field retaining both `syn::Type`
values and its authored span. Store parsed pattern metadata once if validation
needs it. Separate parsing, validation, and emission; the proc-macro entry
point only converts an error to `compile_error!`.

The dialect owns names, field shapes, and mnemonic collisions across its leaves.
The parser derive owns complete pattern binding validation and parser code.
Avoid building a second full pattern compiler inside the dialect macro.

## Implementation tasks

1. Parse declaration/options once. Validate trailing tokens, unsupported
   declaration shapes, missing separators, empty declarations, and attribute
   placement before generating any leaf item.
2. Validate duplicate instruction names structurally, retaining both declaration
   locations. Handle raw identifiers consistently with Step 3’s naming policy.
3. Validate exactly one pattern attribute where supplied. The current code
   searches attributes in reverse and effectively chooses the last pattern;
   make duplicate patterns an explicit error rather than an accidental override.
4. Preserve duplicate mnemonic rejection, including generated-versus-explicit
   names. Generated mnemonics currently lowercase the full identifier:
   `ConstI64` becomes `consti64`, not `const_i64`. Do not silently change source
   syntax to match the older illustrative documents.
5. Keep malformed-pattern diagnostics owned by `vihaco-parser-derive`; avoid
   reporting a guessed mnemonic collision before the malformed pattern error.
   If reliable mnemonic extraction requires shared parser support, expose only
   a small common routine and keep parser crates within edition 2021/MSRV 1.75.
   Do not expand this step into a parser crate rewrite.
6. Emit the runtime structs with their current `Clone`, `Debug`, and `PartialEq`
   derives, documentation, public visibility, and public tuple operands.
   Emit the paired surface structs with syntax operand types and `Parse` derive,
   dialect head, and the supplied pattern.
7. Preserve payload paths authored in the enclosing scope. Use a documented
   definition-scope import bridge or precise rebasing for generated runtime
   and syntax modules; compile `self`/`super` paths rather than assuming broad
   imports handle explicitly relative paths correctly.
8. Build leaf entries in declaration order, with `selector == type_route ==
   dialect::Instruction` and empty source steps. Pass that model to the shared
   leaf-tree and exporter helpers.
9. Emit trees immediately: runtime leaves re-export `dialect::Leaf`; surface
   leaves re-export `dialect::syntax::Leaf`. Honor inventory IDs without changing
   canonical dialect names or source heads.
10. Implement the dialect directly on the current stack base. Where an older
    implementation is already present, replace it. Remove any remaining
    enumerator/selection-arm machinery and update all callers to the same
    shared exporter. Keep no legacy wire-format branch.

## Generated shape

```text
cpu::{Add,Halt}                                      runtime structs
cpu::syntax::{Add,Halt}                              surface structs/parsers
cpu::__private::__types::runtime::cpu::{Add,Halt}     concrete re-exports
cpu::__private::__types::surface::cpu::{Add,Halt}     concrete re-exports
cpu::__private::__entries!                          full ordered response
```

No `Execute` obligation, aggregate enum, operand conversion, instruction-set
association, or bytecode policy belongs in this expansion.

## Tests

Extend `crates/vihaco/tests/dialect_macro.rs` and
`crates/vihaco-runtime/tests/dialect_macro.rs` rather than duplicating their
fixtures in a second implementation suite.

- Unit and tuple leaves, multi-operand leaves, mapped operands, qualified and
  generic payload types, trailing commas, documentation, and raw identifiers.
- Explicit patterns and the exact currently generated lowercase mnemonic.
- Callback capture proving one complete response in declaration order and
  unchanged opaque state; runtime/surface type identity through both trees.
- Nested module and cross-crate use, imported dialect aliases, custom roots,
  custom IDs, and same-named dialects using distinct IDs.
- Errors for empty/named-field declarations, duplicate names and mnemonics,
  duplicate patterns, invalid mappings, unsupported attributes/options,
  trailing tokens, and default backing-macro collisions.
- Preserve the malformed-pattern-before-duplicate fixture’s error ownership.

Use AST assertions for first-expansion tree placement and real compilation
for re-export identity. Review diagnostic changes as intentional improvements.

## Exit criterion

The dialect derives every leaf and inventory entry from one declaration model,
uses shared tree/export helpers, and passes leaf parsing plus downstream
inventory tests through both public roots. The inventory contains no repeated
metadata already owned by the leaf declaration or parser.
