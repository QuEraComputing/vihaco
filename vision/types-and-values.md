# Author-Defined Types and Values

## Status and Direction

Vihaco does not define one built-in guest `Value` enum or `Type` enum. It provides the framework
boundaries through which a machine author supplies values and types:

- Scalar `Parse` implementations and syntax helpers for constructing typed surface products.
- Generic parsed-module and resolved-module containers.
- Generic component, message, effect, and instruction relationships.
- Primitive byte-encoding implementations and composition of author-defined codecs.

The author decides whether a machine needs:

- One scalar value type such as `i64`.
- Several unrelated typed domains.
- A heterogeneous value carrier for a dynamically typed stack.
- Resolved type descriptors for signatures and typed instructions.
- No runtime type descriptor at all because Rust types carry every required distinction.

These choices form an author-defined **data model**. A data model commonly lives in a module or
reusable crate beside the components and composites that use it. A composite commits to concrete
data-model types through its fields and routes, but containment does not make the composite the
semantic owner of those types.

The model has the following properties:

1. Vihaco's scalar parsers are building blocks, not a guest value system.
2. Surface values and types are ordinary author-defined Rust products implementing `Parse`.
3. Parsed modules contain typed surface instructions rather than untyped fallback forms.
4. Module-level parameter and return types use an author-selected surface type.
5. `Resolve` lowers author-defined surface products into author-defined runtime products.
6. Runtime messages and effects use the narrowest useful Rust types.
7. Components that exchange a value use the same boundary type or an explicit conversion.
8. No route performs an implicit cast merely because a value crosses a component boundary.
9. Bytecode serializes a resolved section tree; each section uses codecs for its owner's concrete
   author-defined types.
10. Rust enum layout, `usize`, and implicit variant order never define a persistent bytecode ABI.

## The Three Type Layers

The word “type” refers to three different mechanisms that must remain distinct.

### Rust Types

Rust types establish framework relationships:

```rust
impl Execute<Add> for ArithmeticUnit<i64> {
    type Message = Operands<i64>;
    type Effect = ValueResult<i64>;
    type Fault = ArithmeticFault;

    // ...
}
```

They statically pair an instruction with its component, message, effect, and fault. A mismatched
route should fail to compile whenever the mismatch is visible at this layer.

### Surface Types

Surface types preserve the type syntax written in SST:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(type)]
pub enum CpuSurfaceType {
    #[pattern = "`bool`"]
    Bool,

    #[pattern = "`i64`"]
    I64,

    #[pattern = "`f64`"]
    F64,
}
```

They may also contain unresolved names, type arguments, aliases, address spaces, units, or other
source-level information:

```rust
pub enum MachineSurfaceType {
    Named(QualifiedName),
    Vector {
        element: Box<MachineSurfaceType>,
        length: u32,
    },
}
```

Vihaco does not require one universal surface-type AST. An author uses the smallest product that
faithfully represents the selected SST dialect.

### Runtime Types and Values

Runtime data is whatever the configured machine executes with. A statically typed machine may use
Rust scalars directly and need no guest `Type` enum:

```rust
pub struct NumericMachine {
    stack: Stack<i64>,
    arithmetic: ArithmeticUnit<i64>,
    channel: Channel<i64>,
}
```

A heterogeneous stack machine may define its own carrier:

```rust
pub enum CpuValue {
    Bool(bool),
    I64(i64),
    F64(f64),
    Function(FunctionId),
    Heap(HeapRef),
}

pub enum CpuType {
    Bool,
    I64,
    F64,
    Function,
    Heap,
}
```

Those enums belong to the CPU data model, not to vihaco core. Other machines may reuse them, extend
them through a new author-defined carrier, or avoid them entirely.

## Ownership

Type and value ownership follows semantic definition and composition rather than component
containment:

| Concern | Owner |
|---|---|
| Scalar parsing and byte encoding | Vihaco parser/core libraries |
| Meaning of a domain type such as `HeapRef` or `ChannelId` | The library defining that domain |
| Closed value carrier for a particular architecture | The data-model or machine author |
| Surface grammar for values and types | The surface product that implements `Parse` |
| Module-level surface type | The author-selected SST dialect |
| Source type checking and lowering | `Resolve<ModuleSyntax>` |
| Storage and invariant-preserving mutation | The component |
| Concrete types used by fields and routes | The composite declaration |
| Cross-domain conversion semantics | An explicit author-selected instruction, adapter, or handler |
| Encoding of an author-defined type | The crate defining that type |
| Encoding of the generated machine instruction sum | The composite-generated codec |

A component may define a type when the type is part of its reusable semantic domain. A heap library,
for example, may define `HeapRef`. That does not give each heap instance a private type system.
References that cross component boundaries retain the library-defined identity and any runtime
provenance required to select a valid heap.

A composite does not automatically merge child types into a generated universal `Value` enum. It
selects concrete component instantiations:

```rust
pub struct Machine {
    stack: Stack<CpuValue>,
    heap: Heap<CpuValue, HeapRef>,
    channel: Channel<CpuValue>,
}
```

The same data model may be shared by several composites. Conversely, one composite may contain
multiple independent typed domains when no universal carrier is useful.

## Scalar Building Blocks

Vihaco parser core supplies `Parse` implementations for the scalar source forms it supports, such
as signed and unsigned integers, floating-point numbers, and booleans. Authors compose those
parsers through fields in their own value and instruction products:

```rust
#[derive(vihaco_parser::Parse)]
#[syntax_class(value)]
pub enum CpuLiteral {
    #[pattern = "`i64` `,` $0"]
    I64(i64),

    #[pattern = "`f64` `,` $0"]
    F64(f64),

    #[pattern = "`bool` `,` $0"]
    Bool(bool),
}
```

Scalar parsing is fallible. Out-of-range input returns a parse error and never panics. The set of
supported scalar parsers is an SST API decision; it need not imply that every Rust scalar is a
portable bytecode operand.

Identifiers, quoted strings, and unresolved literal text are distinct lexical products. `String`
must not ambiguously mean all three. Vihaco may provide helpers or newtypes such as:

```rust
pub struct Identifier(pub String);
pub struct StringLiteral(pub String);
pub struct LiteralText(pub String);
```

`LiteralText` is useful when a neighboring surface type determines how a token is interpreted:

```rust
pub struct SurfaceConstant {
    pub ty: CpuSurfaceType,
    pub literal: LiteralText,
}
```

It is deliberately unresolved lexical data, not a framework-owned `SurfaceValue`.

## Parsed Module Shape

Typed surface instructions are the only body items in a parsed module. Unknown or malformed
instructions fail at the parser boundary rather than becoming generic mnemonic/operand records.

Module-level signatures must use an author-selected surface type:

```rust
pub struct ParsedModule<S>
where
    S: ModuleSyntax,
{
    pub header: S::Header,
    pub functions: Vec<ParsedFunction<S>>,
}

pub struct ParsedFunction<S>
where
    S: ModuleSyntax,
{
    pub name: String,
    pub params: Vec<Param<S>>,
    pub return_ty: Option<S::Type>,
    pub body: Vec<S::Instruction>,
}

pub struct Param<Ty> {
    pub name: String,
    pub ty: Ty,
}
```

The exact generic ordering remains an API decision. The required property is that neither function
signatures nor instruction fields depend on a vihaco-defined surface type.

Values usually appear inside surface instruction products and therefore need no parsed-module-wide
value parameter. If SST later gains a module-level constant declaration independent of
instructions, that declaration receives its own author-selected surface value type.

A surface instruction implements `Parse` and the surface-instruction marker. It does not implement
the runtime bytecode `Instruction` contract. The parser derive may generate the marker for
`#[syntax_class(instruction, ...)]`; no surface product should need opcodes or byte codecs merely to
participate in `ParsedModule`.

## Resolution

Types and values follow the same stage boundary as instructions:

```text
SST text
    -> pattern parser
    -> ParsedModule<ModuleSyntax>
    -> Resolve<ModuleSyntax>
    -> Module<RuntimeInstruction, Constant, RuntimeType, Info>
    -> runtime program image
```

The concrete `Constant` and `RuntimeType` parameters are author-defined. Either may be a scalar,
enum, newtype, or unit when the machine does not need that category.

This pipeline describes the contents owned by one SST section. A multi-section SST file applies it
recursively: each section is parsed and resolved by the component or composite selected for that
section path, while the file's global context supplies shared navigation and linkage data. Parent
and child sections may use different surface instructions, surface types, runtime instructions,
constants, and runtime type descriptors.

Resolution performs every transformation requiring source or module context:

- Resolve surface type names, aliases, and parameters.
- Validate function signatures and declarations.
- Interpret unresolved literal text.
- Check numeric ranges and other value invariants.
- Intern strings and constants.
- Resolve functions, labels, channels, and other symbolic identities.
- Select runtime instruction routes for overloaded surface forms.
- Expand sugar into runtime instructions.
- Introduce a conversion only when the selected source language defines one.
- Reject unsupported type/value combinations before execution.

For example:

```text
cpu::const i64, 42
    -> SurfaceConstant { ty: I64, literal: "42" }
    -> resolve and range-check
    -> CpuValue::I64(42)
    -> PushConstant(ConstantId(7))
```

An author may instead parse directly to `CpuLiteral::I64(42)`, moving the type/literal pairing to
the parser. Both are valid. The former permits type-directed literal syntax; the latter makes more
invalid combinations unrepresentable before resolution.

The output contains no unresolved source type names, ambiguous literals, or symbolic references.
Runtime message resolution reads live machine state; it never repeats source type or literal
resolution.

## Runtime Dataflow

Messages and effects use the narrowest useful Rust type. A reusable arithmetic path may be:

```text
Stack<i64>
    -> Operands<i64>
    -> ArithmeticUnit<i64>
    -> ValueResult<i64>
    -> Stack<i64>
```

A heterogeneous stack may resolve and validate a dynamic value before component execution:

```text
Stack<CpuValue>
    -> resolve two CpuValue::I64 operands
    -> Operands<i64>
    -> ArithmeticUnit<i64>
    -> ValueResult<i64>
    -> handle as CpuValue::I64
    -> Stack<CpuValue>
```

This preserves dynamic storage where the architecture requires it while presenting the exact
operand type to `Execute<I>`.

`Undefined` is not required as a universal value or type. Uninitialized storage is normally modeled
as slot state:

```rust
pub enum Slot<V> {
    Uninitialized,
    Initialized(V),
}
```

An author may still define `Undefined` as a real guest value when that is part of the selected
language semantics.

## Cross-Component Compatibility and Conversion

Moving a value between components does not imply conversion. Compatible routes share a Rust
boundary type:

```text
Effect<i64> -> Handler<i64>
```

An incompatible route is rejected:

```text
Effect<i64> -/-> Handler<f64>
```

The author makes conversion explicit through one of:

- A runtime conversion instruction.
- A conversion component.
- A named route handler.
- Resolution-time conversion of a constant.
- Resolution-time insertion of an explicit runtime conversion when the source language specifies
  an implicit coercion.

Conversion semantics are named and testable. Checked, saturating, wrapping, lossy, and bitwise
reinterpretation are not one generic `cast` operation. Vihaco core does not provide a universal
`Value::cast`, and generated message/effect wiring never invents a conversion.

Nested composites follow the same rule. A child exports concrete boundary types. A parent either
uses those types directly or selects an explicit adapter; containment does not erase the
distinction.

## Resolved Constants and Live Values

Program constants and live runtime values need not have the same Rust type.

A serializable constant may contain:

- Scalars.
- Interned string identifiers.
- Function identifiers.
- Immutable aggregate initializers.
- Library-defined static configuration.

A live value may additionally contain:

- Heap references tied to an allocation generation.
- Resource handles.
- Continuation identifiers.
- Device-local references.
- Other state whose meaning exists only after loading.

Ordinary program bytecode must not serialize a live runtime handle accidentally. An author may use
one type when every runtime value is a valid constant, or separate `Constant` and `Value` types when
their invariants differ. Snapshots are a separate format with separate ownership and validation.

## Bytecode Encoding

Bytecode is a serialization of a resolved multi-section program tree for a concrete machine
topology. It is not a serialization of parsed surface syntax or arbitrary Rust memory.

The file container and a section payload have different ownership:

- The file container owns magic, format version, flags, one global context, and the root of a
  recursive section tree.
- The global context owns information intentionally shared across sections, including the mapping
  used to resolve child-section name indices.
- Each section owns one local header, one local bytecode payload, one child table, and its nested
  child sections.
- The component or composite selected by the section path owns the schema of that section's header
  and payload.
- A generated composite loads its own section and forwards named direct child sections to the
  corresponding loadable fields.

The conceptual container shape is:

```text
file header
    magic
    format version
    flags
    global-context length

global context
    section-name table
    optional author-defined global linkage data

root section
    section frame
        total section length
        local header length
    local author-defined header
    local payload length
    local author-defined payload
        local module data
        local instruction stream
    child table
        local child-name index
        child offset relative to this section
    encoded child sections
        recursively use the same section framing
```

The fixed container parses framing and builds section views without interpreting local headers or
payloads. It resolves child names through the global context and validates the recursive ranges.
The selected loader then decodes each section through the concrete types of its target component or
composite.

### Section-Local Data Models

A bytecode file does not imply one `Value`, `Type`, constant, or instruction codec for the entire
section tree. Each section may resolve to a different local module:

```text
root:
    Module<RootInstruction, RootConstant, RootType, RootInfo>

root/cpu_a:
    Module<CpuInstruction, i64, (), CpuInfo>

root/radio:
    Module<RadioInstruction, RadioConstant, RadioType, RadioInfo>
```

The root composite may own a local program in addition to its children, or its local payload may be
empty. Two sibling sections may reuse the same data-model crate and codec, but sharing is explicit
rather than imposed by the file.

Runtime messages and effects may move values between the loaded components. That typed runtime
dataflow does not require their program sections to use one byte-level value representation. If a
source-level reference crosses sections, resolution represents its scope explicitly—for example
with a `SectionPath` plus a section-local identifier, or with an intentionally global identifier
allocated by the global context.

Identifiers state their scope:

```rust
pub struct GlobalStringId(pub u32);
pub struct SectionConstantId(pub u32);
pub struct InstructionIndex(pub u32);
```

An unqualified integer must not be interpreted sometimes as a global index and sometimes as a
section-local index.

Vihaco supplies encoding and decoding contracts and implementations for portable primitives.
Libraries and authors implement or derive them for their own products:

```rust
pub trait Encode {
    fn encode<W: std::io::Write>(&self, output: &mut W) -> eyre::Result<()>;
}

pub trait Decode: Sized {
    fn decode<R: std::io::Read>(input: &mut R) -> eyre::Result<Self>;
}
```

The exact trait names remain an API decision. Encoding is independent from `Parse`: a surface type
may parse without being encodable, and a runtime type may be encodable without having SST syntax.

The ownership chain is:

| Encoded product | Codec owner |
|---|---|
| File and recursive section framing | Vihaco core |
| Global context contents | The selected global-context author |
| Fixed-width scalar | Vihaco core |
| Library newtype such as `ChannelId` | Defining library |
| Section-local value/type product | That section's data-model author |
| Section-local runtime instruction product | That section's instruction author |
| Section-local machine instruction sum and route opcode | The owning composite's generation |
| Section-local header and module metadata | The section owner |

Encoding follows the section tree:

```text
resolve global context
    -> resolve root section with its selected resolver
    -> recursively resolve each admitted child section
    -> encode the global context once
    -> encode each section's local header and payload with its owner
    -> write each parent child table and relative offsets
    -> finish recursive section lengths
```

An author-defined value enum may encode its own discriminant and payload. A machine using only
`i64` constants may need no value tag because the containing schema already determines the payload
type. Vihaco does not require a universal type table or value-tag registry. A self-describing type
table can be added by a data model or later tooling requirement without becoming the semantic
owner of the types.

### Compatibility Identity

The file format version identifies the framing contract, not every local instruction and data-model
schema. One file-wide machine ABI fingerprint is insufficient when nested reusable sections may
evolve independently.

Compatibility can be established at the scopes where schemas are selected:

- The file header identifies the container format.
- The selected global context identifies or validates its own schema when necessary.
- A section payload may carry a section-local schema identity or fingerprint.
- A generated loader may derive the expected section schema from the concrete field selected by the
  section path.
- An optional root topology fingerprint may validate the expected section tree, but does not
  replace section-local validation.

The initial implementation may rely on the statically selected loader for a section's expected
schema. If persistent compatibility across independently versioned component libraries is a goal,
section-local identities should be added to the fixed section envelope rather than hidden in
author payload bytes.

Route opcode values are section-local. The same numeric opcode may identify different runtime
instructions in two sections because the section path selects different decoders. Stable opcode
assignment is required within each section ABI; no file-wide opcode registry is required.

The portable wire rules are:

- Fixed-width integers and identifiers use explicitly selected widths and endianness.
- Persistent fields never use `usize` or Rust enum discriminants.
- Semantic identifiers use newtypes that state whether their scope is global or section-local.
- Booleans have one canonical encoding and reject other values.
- Floating-point values encode their IEEE bit representation under a documented NaN policy.
- Route opcodes are stable section ABI data, not derived implicitly from variant order.
- Variable-sized records carry checked lengths.
- Decoders reject truncated input, trailing payload bytes, invalid tags, invalid indices, and
  configured resource-limit violations.
- Section lengths, header lengths, local payload lengths, child counts, and relative child offsets
  use checked arithmetic.
- Direct child names are unique within their parent, expected by the selected composite, and
  resolved through the global context.
- Child ranges remain inside their parent, begin after the parent data and child table, and do not
  overlap.
- Branch targets refer to instruction indices within their local section unless an architecture
  explicitly chooses another scope.

The loader verifies container framing before exposing the root section view. Recursive composite
loading then validates each section path, local header, local payload schema, child set, and
section-local runtime invariants before constructing executable runtime instructions. Decoding
untrusted bytecode establishes the same per-section invariants that recursive SST resolution
establishes.

## Consequences for the Current Rewrite

The rewrite removes vihaco's built-in runtime `Value` and `Type` enums. It retains or introduces:

1. Fallible primitive `Parse` implementations for the supported SST scalars.
2. Distinct helpers for identifiers, quoted strings, symbols, and unresolved literal text.
3. An author-selected surface type parameter on parsed functions and modules.
4. Typed surface instruction bodies with no generic raw-form fallback.
5. A surface-instruction marker independent of the runtime bytecode instruction trait.
6. Generic resolved modules over runtime instruction, constant, type, and extra metadata.
7. Portable scalar byte codecs and author-defined section-local codecs.
8. Recursive SST resolution and bytecode loading through the selected section owners.

Framework-level placeholders named `SurfaceValue` or `SurfaceType` should not become semantic data
models. A lexical helper may remain under a name that states what it preserves, while a
module-level surface type becomes generic.

Component migrations replace references to vihaco's old enums with:

- A scalar or newtype when the component has one typed domain.
- A generic parameter when the component is reusable across data models.
- An author-defined dynamic carrier when the architecture requires heterogeneous storage.
- Explicit conversion routes where component boundary types differ.

## Verification

The type and value architecture is established when:

- Vihaco exports no required guest `Value` or `Type` enum.
- A machine using only `i64` can parse, resolve, execute, and encode without defining a value enum.
- Another machine can define and use its own heterogeneous value and type enums.
- Two composites can share a data-model crate without copying its definitions.
- A parsed function signature uses the author's surface type.
- Surface instruction types do not implement runtime bytecode traits.
- Unsupported or out-of-range scalar input produces a parse error without panicking.
- Resolution rejects invalid type/literal and instruction/type combinations.
- Cross-component wiring accepts identical boundary types and rejects incompatible ones.
- Every cross-type conversion has explicit author-selected semantics.
- Constants and runtime-only handles cannot be confused accidentally.
- Bytecode round trips a root section and heterogeneous nested child sections.
- Each section may use different author-defined instructions, constants, types, and headers.
- Route opcodes and section-local identifiers are interpreted only in their owning section.
- Global and section-local identifiers cannot be confused accidentally.
- Bytecode compatibility does not depend on Rust variant order, layout, or pointer width.
- Recursive SST and bytecode loading establish equivalent per-section runtime invariants.

## Deferred Questions

The first implementation does not need to decide:

- Whether a common data-model trait usefully packages an author's value and type families.
- Whether generated composite declarations should provide shorthand for repeated data-model
  parameters.
- Whether a future generic tooling format needs self-describing type schemas.
- Whether section-local schema identities belong in fixed section framing or author headers.
- Whether a root topology fingerprint is useful in addition to section-local compatibility checks.
- Whether arbitrary-precision numeric literal helpers belong in vihaco parser core.
- Whether snapshots share any encoding traits with ordinary program bytecode.
- Whether runtime specialization eventually removes dynamic value checks from selected routes.

These questions may improve ergonomics or portability. They do not change the ownership rule:
Vihaco supplies composition and scalar infrastructure; authors define the semantic types and values
their machines use.
