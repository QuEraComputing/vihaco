# Step 3: dialect declarations and instruction pairing

**PR:** [PR 1: typed instruction relationships and dialect declarations](pull-request-plan.md#pr-1-typed-instruction-relationships-and-dialect-declarations).

**Review/commit:** Complete this file's work and report it for user review.
If there are eligible changes outside `design/`, wait for explicit approval
before its dedicated commit. If only design changes exist, record a no-commit
checkpoint and wait for review approval before the next step. See
[workflow](README.md#per-file-review-and-commit-workflow).

**Commit exclusion:** Never stage or commit any file under `design/` at any
point in this plan, including plans, contracts, probes, fixtures, and results.
Exclude them from every implementation commit and PR. Do not move local design
artifacts elsewhere or create an empty commit to satisfy a step boundary.

**Source references:** current checkout and pinned behavioral reference;
see [branch context](README.md#source-references-and-branch-context).
**Prerequisites:** Step 2 committed; Step 1 operand-scope decision accepted.
**Next:** [Step 4](04-components.md).

## Outcome

Preserve marked runtime structs and generate paired surface leaves and parsers,
complete instruction descriptions, and the coverage relationship from one
`#[dialect]` module. The dialect is the authority for its marked leaves,
not a macro exporter.

## Declaration model

Parse the attribute options and one inline `syn::ItemMod`. Keep its ordered
items; collect immediate `#[instruction]` structs into instruction records.
The enclosing `#[dialect]` consumes each marker exactly once; `#[instruction]`
is a helper, not an independent instruction producer. Preserve imports and
unmarked items without treating them as leaves or scanning nested modules.
Each operand retains one authored `syn::Type` and its spans, used in both
runtime and surface fields.
Parse, validate, and emit separately; the public entry point only converts
syn errors into compiler diagnostics.

Use ordinary Rust unit/tuple struct grammar. Require unrestricted `pub` on
marked structs; preserve authored module and field visibility. Preserve runtime
operand paths, docs, meaningful attributes, and existing Clone/Debug/PartialEq
behavior without duplicate derives. Runtime and surface fields share their
authored types; no operand helper, remapping option, or new parser-root attribute
is introduced. Operand types implement Parse through ordinary Rust.
The parser derive continues to own complete pattern binding validation.

## Implementation tasks

1. Parse options/module once; require an inline module with at least one marked
   instruction. Reject out-of-line modules, marked non-struct items,
   named-field/generic instruction structs, repeated markers, trailing input,
   private/restricted marked structs, duplicate patterns, duplicate leaf names, and
   unsupported helper placement with useful spans.
2. Preserve canonical dialect identity and unraw names independently of import
   aliases. Preserve lowercase mnemonic generation and duplicate mnemonic
   validation without shadowing an earlier malformed-pattern error.
3. Retain the authored module and runtime structs, stripping consumed helpers;
   emit hidden pub __InstructionNameSurface structs beside them with Parse,
   reexported through pub syntax as syntax::InstructionName (approved in Step 3 review).
   Keep runtime and surface fields in the same authored scope; no module bridge
   or operand-path rewriting is needed. Root generated Parse/Chumsky references
   through the existing framework-root mechanism or an internal emitter hook;
   do not add a public helper attribute. Add the gated runtime Parse derive
   reexport as needed; preserve independent parser callers and Rust 1.75 syntax.
4. Implement InstructionSpec once for each runtime leaf, associating its surface
   type and nominal dialect identity. Reuse that association everywhere else.
5. Emit a fixed declaration-local hidden dialect identity with its complete
   ordered instruction description. A component macro may append this one known
   internal contract name to an actual dialect module path;
   it never guesses another module from a component type.
6. Emit Coverage<C> directly for the dialect identity, with one C: Execute<I>
   bound per declared leaf. No type-level instruction list or list traversal
   is needed, and no consumer needs the leaves back as macro tokens.
   Include the validated source mnemonic in each leaf's typed association so
   selected parser ordering needs no external token enumeration.
7. Keep helper names scoped locally and validate collisions inside this
   declaration. Same-named dialects in different modules need no exporter IDs.
8. Export the attribute only under `typed-declarations`; gate all output under
   item-level conditions. Follow the accepted raw-leaf
   policy and improve parser helpers only if needed for this public behavior;
   parser crate code stays within its explicit MSRV.
9. Add public leaf documentation/examples through facade and direct runtime.
   Do not emit aggregate runtime enums, From conversions, dispatch,
   availability trees, inventory exporters, or opcode allocation policy.

## Verification

Compile inline dialect modules with marked pub unit/tuple structs and operands,
patterns, docs/derives/visibility, imports and unmarked helper items, nested
declaration scopes, relative/external paths, generic/qself operands, raw dialect
names, and declaration name collisions. Unmarked and nested-module items must
not enter the leaf list. Check paired runtime/surface identity through actual
traits.
Assert descriptor/list order and complete coverage, not hidden AST formatting.

Use same-named dialects in one crate without any user ID and dialect module
import aliases across crates. Capture parser values for canonical syntax.
Review trybuild snapshots for out-of-line/empty modules, unsupported marked
items, private/restricted visibility, misplaced/repeated markers, and
shape/name/pattern/mnemonic/raw-leaf errors.
Repeat the operand-name capture cases proved in Step 1 against the real macro.

## Exit criterion

One annotated inline dialect preserves its marked runtime types and produces
associated pairing, a complete ordered description, and direct coverage bounds.
Public parsing and cross-crate identity work without exported macros or
companion trees.
All affected docs/gates pass; report the dedicated commit for approval.

## End-of-file checkpoint

**Status:** reviewed and committed as c3945f7.
See [Step 3 results](step-3-results.md).
**Dedicated commit:** c3945f7, `feat: generate typed dialect instruction associations`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
