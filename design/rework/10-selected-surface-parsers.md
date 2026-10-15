# Step 10: selected surface parsers

**PR:** [PR 4: authored instruction sets and selected parsers](pull-request-plan.md#pr-4-authored-instruction-sets-and-selected-parsers).

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

**Prerequisites:** [dialect](05-dialect.md) and
[instruction-set selection](09-instruction-set-declarations-and-selection.md).
**Next:** [integration and editor checks](11-diagnostics-cross-crate-and-editor.md).

## Outcome

Build a surface parser from the selected-entry model without embedding inventory
lookup or declaration validation inside parser quote templates. Parse only
selected routes, map canonical and aliased spellings to the same authored
variant, and handle arbitrary practical branch counts.

## Current implementation and lessons

The current [instruction-set finalizer](../../crates/vihaco-runtime-derive/src/instruction_set/mod.rs)
folds source steps around leaf parsers and chains boxed alternatives. It avoids
the tuple-choice limit for instruction alternatives, but alias choices still
use tuples. A branch-ending identifier lookahead protects prefix-related
mnemonics. Keep those behaviors and verify their lexical assumptions with the
SST embedding context.

The surface parser must call the selected leaf’s `Parse` implementation rather
than a child instruction set’s whole parser. Otherwise an omitted instruction
could re-enter through that broader child parser.

Selection here belongs exclusively to the authored instruction-set enum.
Components contribute complete dialects with coverage for every leaf. Their
availability and tree contents are never narrowed by this parser's choices.

## Parser construction

For each `SelectedChoice`, in authored variant order:

1. Construct `<SurfaceLeaf as Parse<'src>>::parser()`.
2. Wrap it with the recorded source steps from innermost to outermost. A step
   accepts its canonical spelling or one of its aliases, each followed by `::`.
3. Add the established token-boundary check needed to stop a short mnemonic
   from consuming the beginning of a longer selected mnemonic.
4. Map the leaf value to the authored surface variant and erase the branch’s
   combinator type with `.boxed()`.
5. Combine branches in authored order through boxed `.or(...)` composition.

Compose alternatives independently within each step. For two source steps with
three spellings each, generate two groups of three alternatives rather than
nine whole parser branches. Use the same unbounded alternative helper for
source aliases and instruction branches; do not leave a tuple-length limit
on a field’s alias list.

## Implementation tasks

Once the parser emitter is complete, activate the new public instruction-set
macro and run Step 9's full macro acceptance cases alongside this step's parser
checks. Keep an already working macro connected throughout any restructuring.

1. Extract parser emission to `instruction_set/parser.rs` with inputs limited
   to selected choices and the framework root. Keep source spelling validation
   in the model/field parser and selection in Step 9.
2. Add helpers for one source-step prefix, one selected leaf branch, and a
   nonempty alternative list. Return `syn::Result` for invalid internal input;
   remove `expect` assumptions reachable through malformed hidden invocations.
3. Emit the existing `Parse<'src>` signature and `SurfaceInstruction` marker
   through the resolved facade/runtime root, including its `chumsky` re-export.
   Keep generated lifetime/helper names local and avoid authored generic names.
4. Preserve boundary and backtracking behavior for unit and operand-bearing
   leaves. Document exactly which following characters count as identifier
   continuations; validate that against current parser/SST grammar before
   changing the existing alphanumeric-or-underscore lookahead.
5. Distinguish a branch’s mnemonic boundary from whole-input termination.
   Standalone parsing must reject trailing junk, but do not append global
   `.end()` inside a parser that must also compose into function bodies and
   multiple-instruction SST source.
6. Keep parser construction deterministic. Do not sort by mnemonic length to
   fix an ordering bug or assume source aliases are Rust module identifiers.
7. Maintain the hand-written surface resolution and runtime dispatch example.
   Use an actual mapped operand type to demonstrate why a blanket `From`
   conversion is insufficient for the general interface.

## Acceptance matrix

| Case | Expected behavior |
| --- | --- |
| Canonical nested field prefixes | Selected variant with the original surface payload. |
| All aliases or mixed aliases/canonical names | The same selected variant and value. |
| Same leaf under sibling fields | Distinct authored variants; each can target the intended field. |
| Available leaf omitted from the authored instruction set | Parser rejects it. |
| Leaf from a dialect omitted by the component | It is unavailable and cannot be selected. |
| Loadable child route | Parent parser rejects it. |
| `halt` and `halt_all` selected in either order | Both parse exactly, without short-prefix capture. |
| Mnemonic with additional identifier suffix | Parser rejects it. |
| Incomplete or malformed operand | Parser rejects it and does not succeed via a shorter alternative. |
| More than 26 instruction alternatives | Compiles and maps both early and late choices correctly. |
| More than 26 source aliases | Compiles without a tuple-choice arity dependency. |
| Multiple instructions in an SST body | Parser composes with the existing module grammar. |

Use explicit patterns for the `halt`/`halt_all` test, since the default generated
name for `HaltAll` is currently `haltall`. Include a selected instruction whose
leaf operands consume punctuation, whitespace, negative numbers, and a mapped
surface type. The parser owns syntax; resolution still belongs to the author.

## Verification and exit criterion

Run `instruction_set.rs`, `instruction_set_many.rs`, the direct-runtime parser
test, and an SST/module parsing integration case. Assert actual surface variants
and operand values, not only `has_output()`.

Exit when every parser branch is derived from a selected entry, source spellings
are composed without Cartesian expansion, practical branch/alias counts avoid
tuple limits, and standalone and embedded parsing both pass. No universal
conversion or route generation is introduced to make the example work.
