# Step 6: authored instruction types and semantic field targets

**PR:** [PR 4: authored instruction sets and selected surface parsing](pull-request-plan.md#pr-4-authored-instruction-sets-and-selected-surface-parsing).

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
**Prerequisites:** Step 5 committed; runtime/dialect/component contracts available.
**Next:** [Step 7](07-selected-parsers.md).

## Test implementation policy

Do not add custom test harnesses that write generated source or build artifacts,
invoke Cargo/rustc subprocesses, or execute generated fixture binaries. This
includes rustc-stdin compilation and nested Cargo feature-graph runners.
Use in-process parsing/emission assertions, ordinary unit/integration tests and
doctests, and established trybuild fixtures for compiler diagnostics. Run
independent feature graphs and checked-in fixture crates as top-level
verification commands or CI jobs, never from inside a test. Do not reintroduce
the removed compilation runners in a later step.

## Outcome

Keep the authored runtime enum and its real instruction payload types.
Generate compiler checks for field traversal, declared support, and associated
surface pairing. Complete capability validation must precede concrete use.

## Declaration model

Use Declaration { machine_type, runtime_enum, surface_ident, choices, root }.
Each Choice stores authored variant/payload syn::Type, canonical field target,
and spans. Preserve choice order. Do not collect an external token inventory
or reconstruct a runtime type from a string selector.

The initial enum is nongeneric/nonempty, with one tuple payload and exactly one
target helper per variant. The machine type may be a concrete generic instance,
type alias, or qualified/associated type accepted by the Step 1 grammar.
Preserve enum/variant attrs that remain valid, and consume target helpers.

## Implementation tasks

1. Parse typed macro options and enum once. Validate missing/repeated targets,
   unsupported conditional membership, malformed shapes, empty target paths,
   source aliases used as targets, and exact syntactic duplicate choices.
2. Retain runtime payload tokens and spans in authored scope. Do not rewrite
   them into self/super paths, associated Runtime projections, or hidden keys.
3. Lower target field identifiers into lookups of the parent's hidden
   `__vihaco_field_<name>` usize constant containing its explicit u8 device code,
   preserving unraw spelling. Project
   <Parent as Field<{ <Parent>::CONSTANT }>>::Child at each segment. Each
   projected child is resolved by Rust from its actual parent field relation.
   The terminal type must provide the actual instruction's declared dialect
   and Execute contract.
4. Emit semantic checks at authored field/type spans. Separate unknown fields,
   excluded loadable fields, undeclared dialects, and missing execution.
   Use diagnostic wrapper traits only where they improve actual errors.
5. Force complete concrete machine validation, not just selected-path checks.
   An overlap in an unselected child leaf must still reject the machine.
6. Validate duplicate selected pairs using the parent-local device-code path and canonical metadata
   from actual instruction types; aliases cannot evade checks. Do not forbid
   the same payload through different field paths. Keep payload identity separate
   from authored choice identity so later execution features can be additive.
   A const-local coherence trait can encode the device-code path as a tuple of standard
   [(); CODE] types; no shared path marker or character-key contract is needed.
7. Emit paired surface variants with payload
   <ActualInstruction as InstructionSpec>::Surface. Define a document-hidden
   pub __EnumNameSurface in the authored scope and reexport it as
   syntax::EnumName, matching the dialect convention. Give the syntax module
   the authored enum's visibility. Preserve docs and payload/root scope; do not
   move the enum definition or rewrite self/super paths. The syntax module is a
   VM surface namespace, not a companion inventory. Sets must be module-scoped;
   separate sets need separate authored modules to avoid syntax collisions.
8. Generate HasInstructionSet for the machine with runtime/surface associations.
   Compiler coherence rejects a second association. Keep resolution, execution,
   and bytecode policy explicit rather than infer them from variant names.
9. Gate new support under `typed-declarations` and keep emission internal until
   Step 7 supplies the complete parser and activates
   the public attributes together. No public half-set or dummy Parse impl.
   Tests inspect parsing and generated ASTs in process. Add semantic compiler
   coverage through ordinary public-macro integration tests and established
   trybuild fixtures when Step 7 activates the complete contract.

## Verification

Type-assert actual enum payload and associated surface identity across crates,
renamed imports, concrete aliases, nested generic machines, and qself payloads.
Check original runtime enum paths through hover/definition output-shape probes.

Reject unknown/ordinary/loadable target fields, composite terminal ambiguity,
undeclared dialects despite Execute, duplicate pairs through type aliases,
complete unselected overlaps, malformed helpers/options, and two set associations.
Keep missing generic coverage tests owned by components; selection must not
weaken them or invent a global membership search.

## Exit criterion

Internal set emission preserves real authored payloads and proves the exact
target/type pairing against full validated concrete capabilities. Paired enums
and association have reviewed in-process AST coverage. Public-macro compiler
coverage and parser/public activation remain Steps 7–8.
Report the tested preparation and wait for its dedicated commit approval.

## End-of-file checkpoint

**Status:** approved and committed as `7e33c88`.
See [Step 6 results](step-6-results.md).
**Proposed dedicated commit:** `feat: prepare instruction selection by type and field`.
Report completed implementation, eligible files, checks, and material limits;
wait for explicit approval before committing. If the final public change is
breaking, adjust the conventional marker/footer to the reviewed diff.
Never stage or commit anything under `design/`. A completed report is not
commit approval, and no next step starts while this checkpoint is pending.
