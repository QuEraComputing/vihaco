# Instruction phases and capability-based lowering

Status: foundational implementation with the CPU compatibility migration needed
to keep feature-enabled builds, tests, and doctests working. This document
records the agreed representation and loader contract.

## Purpose

Define each instruction once, retaining parsed operands until a program loader
can lower them into runtime operands. Generate phase-dependent representations
only for instructions whose pattern binding types differ from their runtime
field types. Instructions with identical representations pass through lowering
unchanged.

Each instruction declares the loader capabilities it needs through ordinary
Rust trait bounds. A generated aggregate collects those requirements and Rust
checks that the loader context provides them.

This design introduces syntax-phase instruction values alongside runtime values,
replacing the earlier direct runtime parsing model for differing operands.

## Authored declaration

The `dialect!` frontend accepts only nongeneric instruction declarations.
Dialect authors cannot declare generic instructions or supply their own phase
parameters. The only instruction generics are phase parameters introduced by
the macro when operand types differ. This restriction remains part of frontend
validation.

An instruction's fields describe its runtime operands. Pattern annotations
describe the corresponding syntax operands:

```rust,ignore
#[instruction]
#[pattern = "'call $0:String `,` $1"]
pub struct Call(pub u32, pub u32);
```

Here the first operand is a function name in source and a function index at
runtime. The second operand is an argument count with type `u32` in both phases.
`String` is illustrative; a real dialect may use a dedicated symbol type.

## Generated instruction parameter traits

The framework provides shared `Syntax` and `Runtime` marker types. For an
instruction with differing operands, the macro generates a hidden public trait
that supplies the types of those operands:

```rust,ignore
#[doc(hidden)]
pub trait __CallTypes {
    type Param0;
}

impl __CallTypes for Syntax {
    type Param0 = String;
}

impl __CallTypes for Runtime {
    type Param0 = u32;
}

pub struct Call<P: __CallTypes = Runtime>(
    pub P::Param0,
    pub u32,
);
```

The result has two forms:

- `Call<Syntax>` holds a function name and argument count.
- `Call<Runtime>` holds a function index and argument count.
- `Call` in a type position defaults to `Call<Runtime>`.

Dispatch and Step generation stay unchanged. Their existing instruction type
references select the default `Runtime` phase, preserving current execution
behavior. Only parsing and lowering need to select `Syntax` explicitly.

Each differing operand gets its own associated type, such as `Param0` or
`Param2`. Unchanged operands retain their concrete field types. Associated type
names based on tuple positions avoid inventing semantic names for anonymous
fields.

No `PhantomData` field is necessary: the associated field types already use `P`.
The generated trait must be accessible to downstream users of public
instructions, so it is public with `#[doc(hidden)]`, rather than private.
The generated trait name is `__<Instruction>Types`, such as `__CallTypes`.
Distinct instruction names produce distinct trait names in their declaration
scope. Rust rejects duplicate type names, including a user declaration that
conflicts with a generated trait name; no separate collision mechanism is
needed.

## Selective generation

For each pattern binding, the proposed generation rules are:

| Binding | Syntax operand type | Generated field |
| --- | --- | --- |
| No type annotation | Runtime field type | Concrete runtime field type |
| Annotation matches the runtime field type | Runtime field type | Concrete runtime field type |
| Annotation differs from the runtime field type | Annotated type | Associated type selected by phase |

If no operands differ, the instruction stays nongeneric:

```rust,ignore
#[instruction]
#[pattern = "'load_i32 $0"]
pub struct LoadI32(pub u32);
```

Pattern parsing retains syntax operands without converting them into runtime
operands. This fully replaces the existing generated `TryInto` conversion.
Every instruction with differing syntax and runtime operand types must implement
lowering, including literal conversions such as `I32Literal` to `Word`. A local
conversion that requires no loader services can implement `Lower<C>` for any
`C: ?Sized`; it still takes place during lowering rather than parsing. There is
no implicit conversion fallback or separate immediate-conversion mode.

A procedural macro can compare written type syntax, but cannot establish type
equality through aliases, imports, or associated type normalization. The
implementation compares parsed Rust type syntax (`syn::Type` equality).
Differently spelled equivalent types therefore receive phase representations
and require lowering too, even when their underlying Rust types are equal.

## Instruction lowering and capabilities

The framework exposes a lowering trait:

```rust,ignore
pub trait Lower<C: ?Sized> {
    type Runtime;

    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime>;
}
```

Capabilities are ordinary traits implemented by the loader or its prepared
context:

```rust,ignore
pub trait FunctionLookup {
    fn function_index(&self, name: &str) -> eyre::Result<u32>;
}

pub trait StringIntern {
    fn intern_string(&mut self, value: &str) -> eyre::Result<u32>;
}
```

An instruction author supplies its conversion and declares its requirements
through bounds on `C`:

```rust,ignore
impl<C: FunctionLookup + ?Sized> Lower<C> for Call<Syntax> {
    type Runtime = Call<Runtime>;

    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(Call(context.function_index(&self.0)?, self.1))
    }
}
```

An instruction needing several capabilities uses a combined bound, for example
`C: FunctionLookup + StringIntern + ?Sized`. The parameter trait selects operand
types; the context bounds specify the services needed to convert their values.

Lowering policy belongs to the instruction author. This is sufficient for the
intended API: composite authors reuse the dialect's lowering implementations,
and their program loader supplies the required capabilities. Consumer-owned
resolver policies and destination-specific lowering implementations are outside
this design.

Write authored `Lower` implementations outside the dialect module, using paths
such as `cpu::Call<Syntax>`. They can live beside the dialect declaration or in
a separate lowering module in the instruction's crate. The authored dialect
module then contains instruction definitions and their operand imports; lowering
behavior and capability bounds remain separate. This requires no macro change.

## Identity lowering

For each instruction whose representation is unchanged, the macro generates an
identity implementation:

```rust,ignore
impl<C: ?Sized> Lower<C> for LoadI32 {
    type Runtime = Self;

    fn lower(self, _: &mut C) -> eyre::Result<Self> {
        Ok(self)
    }
}
```

This moves the instruction through unchanged, without requiring capabilities.
Generate an implementation for each unchanged instruction. A blanket identity
implementation for every type would overlap with custom lowering
implementations.

## Generated aggregate enum

A single generated enum can represent both phases. Its hidden phase trait
collects the parameter traits required by its phase-dependent leaves:

```rust,ignore
#[doc(hidden)]
pub trait __InstructionsPhase:
    __CallTypes + __BranchTypes + __ConstStringTypes
{}

impl __InstructionsPhase for Syntax {}
impl __InstructionsPhase for Runtime {}

pub enum Instructions<P: __InstructionsPhase = Runtime> {
    Call(Call<P>),
    Branch(Branch<P>),
    ConstString(ConstString<P>),
    LoadI32(LoadI32),
    AddI32(AddI32),
    Halt(Halt),
}
```

The example assumes corresponding generated parameter traits and instruction
structs for `Branch` and `ConstString`. Unchanged leaves stay nongeneric. The
implementation gives every nonempty aggregate a phase parameter, using indexed
associated instruction types resolved through its inventory. Empty aggregates
remain nongeneric and uninhabited. This works across private and reexported
dialects without exposing their parameter-trait paths.

Aggregate lowering is generated with a bound for each leaf:

```rust,ignore
impl<C: ?Sized> Lower<C> for Instructions<Syntax>
where
    Call<Syntax>: Lower<C, Runtime = Call<Runtime>>,
    Branch<Syntax>: Lower<C, Runtime = Branch<Runtime>>,
    ConstString<Syntax>: Lower<C, Runtime = ConstString<Runtime>>,
    LoadI32: Lower<C, Runtime = LoadI32>,
    AddI32: Lower<C, Runtime = AddI32>,
    Halt: Lower<C, Runtime = Halt>,
{
    type Runtime = Instructions<Runtime>;

    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        match self {
            Self::Call(i) => Ok(Instructions::Call(i.lower(context)?)),
            Self::Branch(i) => Ok(Instructions::Branch(i.lower(context)?)),
            Self::ConstString(i) =>
                Ok(Instructions::ConstString(i.lower(context)?)),
            Self::LoadI32(i) => Ok(Instructions::LoadI32(i.lower(context)?)),
            Self::AddI32(i) => Ok(Instructions::AddI32(i.lower(context)?)),
            Self::Halt(i) => Ok(Instructions::Halt(i.lower(context)?)),
        }
    }
}
```

These bounds collect the capabilities required by every instruction in the
aggregate, regardless of which variants a particular program happens to use.
Missing capability implementations produce compile-time errors when the context
is used to lower that aggregate.

For composites, this aggregate and its lowering implementation are generated
from the instructions supplied by their components' dialects. Composite authors
do not write leaf conversions, aggregate lowering matches, or repeat capability
bounds. Lowering is available automatically whenever their program loader
provides the capabilities required by the included instructions' `Lower`
implementations. Repeated occurrences of an instruction reuse its instruction-
owned lowering implementation; generated lowering wraps the result in the
corresponding runtime variant.

The implementation uses one generic aggregate, with `Runtime` as its default.
The generated aggregate phase trait maps each inventory slot to the appropriate
leaf type. Its parser is available when every leaf in the selected phase has a
parser; unchanged leaves can therefore retain their existing runtime parsing.

## Loader responsibilities

The intended pipeline is:

```text
source
  → Instructions<Syntax>
  → module preparation and capability context
  → instruction lowering
  → Instructions<Runtime>
  → encoding or execution
```

The existing module-level `Resolve` operation can prepare symbol tables,
calculate addresses, construct function metadata, and invoke the leaf lowering
hooks. A loader may implement the capabilities itself or pass a separate context
containing the prepared state.

The loader entry point should require the generated syntax aggregate to
implement `Lower` for that loader or its context, with the corresponding runtime
aggregate as its output. The generated aggregate's leaf bounds then enforce all
required capabilities without a handwritten composite-level capability list.

The SST loading path uses `LoadOwnSstSection::lower_instructions`. After
parsing the section and preparing its symbols and addresses, the loader calls
this provided method before storing runtime instructions in its program image.
The method consumes an instruction iterator and returns `eyre::Result<Vec<I::Runtime>>`
under the bound `I: Lower<Self>`. Composite authors inherit the generated leaf
delegation and the shared conversion loop. Section parsing and program storage
remain the responsibilities of the existing `load_own_sst_section` hook.

The loader's SST loading implementation requires this aggregate bound, using
the actual generated aggregate types:

```rust,ignore
SyntaxInstructions: Lower<Loader, Runtime = RuntimeInstructions>
```

If the loader supplies a separate prepared context, use that context type in
place of `Loader`. This single bound enforces all included instructions'
capability requirements through the generated aggregate `Lower` implementation.

Generate that aggregate implementation alongside the composite instruction enum
in `inventory.rs::aggregate`. Keep `composite_loader.rs` responsible for section
routing: each `LoadOwnSstSection` path lowers its own section with its loader,
then generated forwarding loads child sections through their respective loaders.
Bytecode loading already consumes runtime instructions and needs no lowering
step.

The leaf signature handles one syntax instruction becoming one runtime
instruction. Metadata that disappears and syntax that expands into multiple
instructions need a separate module-level contract. Address calculation and
forward references must be prepared consistently before address-dependent
lowering runs.

## Foundational implementation and verification

- `vihaco-abi` owns `Syntax`, `Runtime`, and `Lower`. The runtime and facade
  reexport them so module loading does not introduce a dependency cycle.
- The dialect frontend continues to reject authored generics. Its internal
  parser emitter generates `__<Instruction>Types` and phase parameters only
  when annotations differ from runtime fields. Unchanged leaves get identity
  lowering. Generated parameter names avoid capturing authored type names.
- Generated phase type traits require associated operands to implement `Clone`,
  `Debug`, and `PartialEq`, matching the existing aggregate requirements.
  Marker bounds also support the instruction's derives.
- Pattern parsing no longer generates `TryInto`. Standalone `Parse` derives
  retain the annotated parser output and require it to match the authored field.
  Dialect parsing constructs the generated syntax specialization instead.
- `InstructionSpec::Syntax` associates each runtime leaf with its parsed form.
  `HasInstructionSet::Syntax` exposes the composite's generated syntax aggregate.
- Composite inventory generation emits aggregate phase mappings, parsing, and
  `Lower` delegation. Dispatch and Step generation continue to select runtime
  defaults. Loadable children remain separate section boundaries.
- `LoadOwnSstSection::lower_instructions` provides the shared lowering loop.
  The loader's aggregate bound enforces all leaf capabilities. Errors return no
  partial code vector; capability implementations may still have side effects
  before an error, such as interning a value.
- Tests exercise differing and identical bindings, reordered multiple operands,
  context-free conversion, capability-based conversion, identity lowering,
  nested/repeated destinations, loader integration, and cross-crate aliases.
  Trybuild pins missing capabilities and missing lowering, including the absence
  of an implicit fallback when `TryInto` exists. Existing authored-generic
  rejection and dispatch coverage fixtures remain in place.

The foundational change includes the minimal CPU literal compatibility edits:
`I32Literal`, `I64Literal`, `F32Literal`, and `F64Literal` are public and derive
`Clone`, `Debug`, and `PartialEq`. They now appear in public generated syntax
type associations, whose operand bounds require those traits. Include
`crates/vihaco-cpu/src/literal.rs` in the foundational commit so the typed CPU
library continues to compile.

The same commit includes CPU instruction-owned literal conversion and migration
of its parser consumers and doctest. Deferring those changes would leave typed
CPU tests broken because parsing now retains syntax values. The standalone
typed CPU example has been removed, along with its Cargo target and README
command. Typed declarations remain opt-in for the runtime and CPU crates.
