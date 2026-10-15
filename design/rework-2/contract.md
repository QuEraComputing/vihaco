# VM declaration contract

**Status:** Step 1 approved; Step 2 runtime support is implemented and awaiting
review. Declaration macros remain planned. See [Step 2 results](step-2-results.md).
**Updated:** 2026-10-01.
**Commit exclusion:** Never stage or commit any file under `design/`,
including this contract, its probes, or results.

## Public concepts and syntax

The supported concepts are instruction dialects, canonical instruction types,
components, device fields, source aliases, loadable sections, and executable
instruction sets. Compiler helpers have no authored declaration role.

- `#[dialect]` annotates an inline Rust module. `#[instruction]` marks its
  immediate unrestricted `pub` unit/tuple structs as runtime instructions, in
  declaration order;
  the enclosing attribute consumes these markers and generates paired surface
  structs, parsing, and complete dialect metadata. Ordinary unmarked module
  items remain ordinary Rust items and do not enter the instruction list.
  The module may have any Rust visibility; instruction structs must be `pub`.
  Docs, meaningful attributes, field visibility, and operand type paths retain
  their authored Rust scope.
- `#[component]` consumes exactly one nonempty
  `#[instructions { arithmetic, control }]` list of dialect module paths.
  Complete dialects are the only component selection form.
- `#[composite]` marks named device fields with `#[device(code, ...)]`.
  An explicit u8 code is required on every device, including loadable sections,
  and codes are unique within each parent. Eligible fields use that code as
  their Field key and hidden field-name constant value; field order does not
  assign keys. Source aliases remain independent of keys. Ordinary fields
  contribute no instruction availability.
- `#[loadable]` remains a device-field section boundary. The parent does not
  expose or target instructions inside that section. Loader interfaces still
  determine whether the child is otherwise valid.
- `#[instruction_set(composite = Machine)]` annotates a nonempty nongeneric
  enum. Each variant has one actual Rust instruction type as its tuple payload
  and exactly one `#[target(controller::cpu)]` device-field path.
  The machine may be a concrete generic type or an imported/type alias.
- Field keys are explicit parent-local device codes, emitted as literal usize
  values in Field impls and generic descriptions. Hidden
  __vihaco_field_<name> usize constants contain those same codes. Codes may be
  sparse, including 0 and 255. Complete descriptions retain eligible declaration
  order, independently of numeric key order. Loadable devices retain codes as
  metadata but have neither Field impls nor target constants.
- Targets contain canonical field identifiers, not source aliases or Rust module
  roots. Raw field identifiers use their unraw identity and source name.
  Actual payload types use ordinary Rust type syntax and compiler resolution.

The dialect declaration shape is:

```rust,ignore
#[dialect]
mod foo {
    #[instruction]
    pub struct A;

    #[instruction]
    pub struct B;
}
```

Tuple instructions use ordinary Rust fields, for example
`#[instruction] pub struct Add(pub i64);`. Explicit patterns remain supported
on marked structs. Runtime and generated surface fields use the same authored
Rust type. There is no `#[operand]` helper, field-type remapping option, or
replacement attribute. Operand types provide ordinary Parse implementations;
application Resolve code handles any later lowering. The old `Syntax => Runtime`
token grammar is not Rust struct syntax.
Initially require at least one marked instruction; reject out-of-line dialect
modules, marked non-struct items, named-field/generic instruction structs, and
repeated instruction markers with spanned diagnostics. Reject private and
restricted visibility (`pub(crate)`, `pub(super)`, and `pub(in ...)`) on marked
instructions; never promote visibility implicitly. Do not recursively
discover instructions in nested modules.

The [Step 1 report](step-1-results.md#proposed-grammar) records exact helper
grammar, public surface-type naming, and the feature boundary.
Do not reuse `inventory`, `inventory_id`,
`__types`, or `__selectors` as authored API.

## Capability versus executable selection

A component provides every leaf of every declared dialect, in list/declaration
order, and must execute them all under its original generic bounds.
An instruction set does not weaken that obligation.

A composite describes all eligible fields in field order and preserves each
child's order. A child executable set does not limit this description.
Different field destinations may accept the same canonical instruction type.

An instruction set selects individual (target, instruction) pairs in enum
order. In the base system, selecting the same pair twice is an error, including
through type aliases. Distinct targets carrying the same type are valid.
Keep payload identity separate from authored variant identity; do not build
a payload-type uniqueness rule that prevents later additive execution features.

Implementing `Execute<I>` without declaring I's dialect does not grant
availability. A target must finish at an execution-capable component, not an
unresolved composite with several possible descendants.

## Instruction and dialect identity

Every marked runtime leaf has one associated surface type, one nominal
dialect identity, and canonical dialect/leaf names. Import and type aliases
preserve those associations.

Generate one Coverage<Component> impl on the dialect identity with an Execute
bound for every declared instruction. No consumer needs a type-level instruction
list; the dialect description retains source order. Instructions must still be
unrestricted `pub` as an authored API rule. A private dialect module is valid.

Repeating a dialect through aliases is invalid. Distinct dialect declarations
with the same canonical head and disjoint leaf names are valid. Overlapping
canonical selectors anywhere in complete component capabilities are invalid,
even when downstream sets omit the overlap.

Annotated dialect modules are the authoritative producers. Manually
implementing hidden inventory/identity traits is not a supported extension API.
Generated internal contracts may change with their owning macros.
Nominal type identity plus generated associations and complete selector checks
replace independent arbitrary-producer type-identity protocols. Do not use
const TypeId equality, hashes, or token string comparisons as identity proofs.

## Runtime and surface types

Preserve the authored dialect module and marked runtime structs in their
original scope, including imports and unmarked helper items. Generate paired
surface structs as document-hidden `__InstructionNameSurface` beside their
runtime structs, with unrestricted `pub` visibility. Reexport them through a
public `syntax` module as `syntax::InstructionName`, including constructors.
Preserve field visibility. Reject collisions with the generated namespace and
internal names. The syntax module contains reexports, so authored operand types
retain their original scope without an operand bridge or path rewriting.
This naming amendment was approved during Step 3 review on 2026-10-01.

Preserve the authored runtime enum and its actual payload tokens in their
original scope. Preserve docs, visibility, derives, variant attributes that
remain meaningful, and Rust's normal enum naming behavior.

Surface variants use each payload's associated surface type and preserve
authored variant identity. As with dialect instructions, define the paired enum
as a document-hidden unrestricted `pub __EnumNameSurface` beside its runtime
enum, then reexport it as `syntax::EnumName`. The syntax module has the authored
runtime enum's visibility and contains a public reexport; original payload
paths and framework roots keep their authored scope. There is no public
SurfaceInstruction suffix or surface-name override. Reject an authored enum
named `syntax`, which would collide with the generated module. Separate sets
in the same Rust module would also collide on `syntax`; author them in separate
modules rather than adding a namespace override.

Instruction sets with this syntax-module reexport must be authored at module
scope. A Rust module cannot reexport a function-local enum from `super`.
Function-local component/composite declarations remain separate supported cases.
This naming/scope amendment was requested by the user during Step 6 review.

The authored set owns the composite's `HasInstructionSet` association.
Reject multiple associations through compiler trait coherence.
No component-generated aggregate enum is needed for availability.

Surface-to-runtime resolution remains explicit application code. Operand
lowering can require interning, symbols, or expansion into multiple runtime
instructions; do not generate blanket `From` conversion or implicit dispatch.
No declaration attribute specifies a second operand type. Preserve existing
runtime/bytecode semantics when migrating callers.

## Source parsing

Each branch parses its canonical target prefixes or declared field aliases,
then invokes only its instruction's associated surface parser. Field separators
are `::`; the dialect owns its `head.mnemonic` spelling and operands.
Source aliases do not alter target or runtime identity.

Generated mnemonics retain the reference's full lowercase spelling:
`ConstI64` becomes `consti64`. Duplicate patterns and mnemonics are errors.
Raw dialect/field names use unraw source spelling. Raw leaf declarations must
either work through the real parser derive or fail with a spanned diagnostic;
never preserve a proc-macro panic.

Parsing accepts exactly the selected instruction branches. It does not parse
a child's whole enum, infer execution behavior, or resolve runtime operands.
Preserve authored variant/description order; stably prioritize longer source
mnemonics when combining selected parsers, as the existing parser derive does.
Read that mnemonic from each actual instruction's hidden typed association.

## Validation boundaries

Macros reject invalid facts visible in their declaration: shape, helper
duplication, exact duplicate names, field/source spelling collisions, malformed
options, and inconsistent section metadata.

Rust proves actual type relationships, declared dialect support, complete
execution coverage, and field-type relationships. Keep generic obligations in a checked
function body under original bounds; do not add required execution bounds
merely to make the check compile.

Static descriptions validate complete canonical-selector uniqueness across
dialects. Source-name syntax and local alias collisions belong to the owning
macros, using standard string and character operations. Cross-dialect selector
validation must be deliberately forced:

- at nongeneric declaration points where the entire description is known;
- for component dialect lists whose identity is independent of component state;
- at concrete instruction-set association, across the whole machine description;
- at internal capability-reporting entry points for concrete generic types.

Unused generic associated constants do not establish validation. Do not claim
all possible generic instantiations were validated at definition time.
Step 1 must prove the chosen forcing strategy, including cargo check/build
differences. No user must call an inventory validator to use a declaration. Reporting helpers
and their row types remain under __private; no public reporting API is added.
The [proofs](step-1-results.md#mechanism-results) establish declaration/concrete
const forcing in metadata checking and generic reporting validation at codegen.

## Scope, attributes, and generics

Item-level `cfg`/`cfg_attr` gates the whole expansion. Preserve unrelated
attributes and consume helpers exactly once. Initially reject conditional
device/loadable fields, conditional dialect leaves, and conditional set variants;
support them later only with coherent coverage/metadata/parser generation.

Components and composites preserve lifetimes, type/const parameters, defaults,
associated-type fields, and original where clauses. The new relationships
do not require inferable module paths for those field types.
Function-local declarations are not rejected merely because old sibling
namespaces could not reach them; Step 1 proves the supported ordinary Rust
scope boundary and records any remaining limitation.

Dialect runtime operands resolve in the authored module's ordinary Rust scope.
Generated surface structs resolve their fields in that same module, including
self/super, imports, explicit parent paths, and associated/generic operand
types. Do not introduce a module bridge or rewrite runtime fields.

## Feature boundary and stabilization

All new declaration APIs, generated support, and runtime relationships are
behind one nondefault Cargo feature, `typed-declarations`. Forward it coherently
from facade to runtime to derive; opting in must enable the complete supporting
stack. Independently useful root/parser fixes may preserve existing behavior
without introducing public attributes. Keep the existing `#[vihaco(crate = ...)]`
root override; do not add a parser-root attribute.

With the feature off, current macros, marker bounds, CPU bytecode, dispatch,
loaders, observers, and guides retain their existing behavior. With the feature
on, the new Component/Composite markers need no HasInstructionSet supertrait;
new `#[composite]` behavior and `#[instruction_set]` activate together at Step 7.
Earlier steps expose only complete APIs and keep composite/set preparation
internal. Cargo unification may enable the feature through another dependency;
that selects the experimental contract for the shared package instance. Test
and document that boundary explicitly; do not promise mixed contracts in one
instance.

Do not add a legacy macro export, compatibility wrapper, adapter, or automatic
old-to-new declaration translation. Existing default code is temporary while
the new implementation stabilizes. After stabilization, migrate its consumers,
remove the old implementation, and retire the feature gate in a deliberate
breaking release. The two feature modes are not a permanent compatibility layer.

This plan implements declaration feature parity with the inventory-based
proposal. Static topology, profiles, backends, drivers, wires, message builders,
continuations, route work, and route/export/source/work attributes from
`../vihaco-static-top-experiment` are deferred additive work. Field relations
provide child-type projections and source names for target helpers. Mutable
field access is deferred until a future feature needs it.

Later execution features are outside this plan. Complete child availability
remains independent of a child's executable instruction set.

## End-of-file checkpoint

This contract has local mechanism evidence and proposed grammar/migration
decisions updated to the user's 2026-10-01 requirements. Step 1 is approved;
[Step 2 runtime support](step-2-results.md) is implemented and ready for review.
The declaration macros remain future steps. All content here remains local
and excluded from commits/PRs.
