---
layout: ../../layouts/Guide.astro
title: Verification
slug: verification
description: "Check programs before they run: typing rules for components, and verified images for embedders."
---

# Verification

Verification checks a whole program before it runs. A verified program can't
hand an instruction an operand of the wrong type, underflow the operand stack,
call a function with the wrong arguments, return the wrong values, or reach a
join point with an inconsistent stack. The runtime still checks heap tags,
bounds, and freed objects, because some facts only exist at run time.

Each component supplies a **typing rule** for its instructions, and the
framework does the rest: it finds the basic blocks of each function, walks them
to a fixpoint, merges stack states where paths join, and reports every problem
at once. A machine can only be verified if every instruction in it has a rule,
and the compiler checks that.

Verification lives in the `vihaco::verifier` module (the `vihaco-verifier`
crate). The `program-image-execution` feature includes it; enable the
`verifier` feature to use it on its own.

## Verifying a program

An [image](/guide/loadables) only accepts a `Verified` module, so verification
is part of loading. (With `typed-declarations` alone, without
`program-image-execution`, programs run through the legacy `ProgramImage`,
which does not require `Verified`: there, verification happens only if you call
`verify_module` before loading.)

```rust,ignore
let module = metadata.into_module(code)?;   // parse and lower as usual
let image = Image::new(Verified::new(module)?);
let mut loaded = Loadable::new(image, machine);
```

If the program has any error, `Verified::new` returns a `VerifyFailed`: it
gives the module back (`failed.module`, or `into_parts()`) together with the
`Rejection`, whose `diagnostics` list every finding, sorted by function and
address. It converts into an `eyre::Report` for any instruction set, so `?`
works; that report shows indices (`function 0, address 2`). To show the module's
names, which every `#[composite]` instruction set provides, render it with
`failed.display_named()`:

```rust,ignore
let verified = Verified::new(module)
    .map_err(|failed| eyre::eyre!("{}", failed.display_named()))?;
```

which reports something like:

```text
verification failed with 1 error
error: operand 0 of `mixer::mixer::scale_f64`: expected f64, found u64
  --> @main+2 (address 2): mixer::mixer::scale_f64
```

Use `Verified::verify` to choose options and keep the result: it returns the
verified module together with a `Verification` holding any warnings (code no
path reaches is reported as `Unreachable`, a warning, not an error) and, if you
asked for them, type maps.

The example below gives a small mixing device typing rules, then verifies one
good and one bad program. Setup that isn't about verification (parsing the
source, and execution, which this page doesn't need) is folded away.

```rust
use vihaco::{
    Execute, HasInstructionSet, Type, Value, Verified, Verify, component, composite, dialect,
    module::{LocalModule, ProgramMetadata},
    verifier::{DiagnosticKind, Role, TypeSlot, TypedSlot, TypedStack, VerifyOptions, stack_rules},
};
# use vihaco::{
#     NoContext, SstFile,
#     loader::MetadataLoweringContext,
#     syntax::{HeaderDeclarations, ParsedModule, lower_functions_with_metadata},
# };

#[dialect]
pub mod mixer {
    /// `[f64, f64] -> [f64]`: multiply two samples.
    #[instruction]
    #[pattern = "'scale_f64"]
    pub struct ScaleF64;

    /// Pop `n` samples and push their sum. The count is an immediate operand.
    #[instruction]
    #[pattern = "'sum_f64 $0"]
    pub struct SumF64(pub u32);
}

#[component]
#[instructions { mixer }]
#[derive(Default)]
pub struct Mixer;
# impl<I> Execute<I> for Mixer {
#     type Message = ();
#     type Effect = ();
#     type Error = std::convert::Infallible;
#     fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> {
#         Ok(())
#     }
# }

// A fixed stack effect is one line.
stack_rules! {
    mixer::ScaleF64: [Type::F64, Type::F64] -> [Type::F64];
}

// Anything else is a `Verify` impl, bounded on the capabilities it uses.
impl<C: TypedStack + ?Sized> Verify<C> for mixer::SumF64 {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        for (index, sample) in cx.pop_n(self.0 as usize)?.iter().enumerate() {
            sample
                .check(&Type::F64)
                .map_err(|error| error.with_role(Role::Operand(index as u32)))?;
        }
        cx.push(C::Slot::of_type(Type::F64));
        Ok(())
    }
}

#[composite]
pub struct Machine {
    #[device(0)]
    #[program]
    execution: vihaco_stdlib::ExecutionState,
    #[device(1)]
    mixer: Mixer,
}

type Program = LocalModule<<Machine as HasInstructionSet>::Runtime, Value, Type>;

# /// Parse and lower a one-function program returning one `f64`.
# fn load(body: &[&str]) -> eyre::Result<Program> {
#     let source = format!(
#         "sst v{}
# .section(root):
# .header(root):
# metadata {{
#     strings: [\"main\", \"mix.sst\"],
#     functions: [{{ name: 0, signature: {{ params: [], ret: [f64] }}, locals: [],
#                    start_address: 0, end_address: {}, file: 1 }}],
#     structs: [], types: [], labels: [], constants: [], main_function: 0,
#     source_symbols: [], extra: {{}},
# }};
# .header(root).
# .text(root):
# fn @main() -> f64 {{
# {}
# }}
# .text(root).
# .section(root).
# ",
#         vihaco::VERSION,
#         body.len(),
#         body.join("\n"),
#     );
#     let file = SstFile::<NoContext>::from_text(&source)?;
#     let ParsedModule { header, functions } = ParsedModule::<
#         <Machine as HasInstructionSet>::Syntax,
#         vihaco::module::SourceType,
#         HeaderDeclarations<ProgramMetadata>,
#     >::parse_section(file.root())?;
#     let metadata = header.into_iter().next().unwrap().resolve_types::<Type>()?;
#     let mut context = MetadataLoweringContext::new(&metadata)?;
#     let code = lower_functions_with_metadata(functions, &metadata, &mut context)?;
#     metadata.into_module(code)
# }
fn main() -> eyre::Result<()> {
    let good = load(&[
        "execution::program::const_f64 1.5",
        "execution::program::const_f64 2.0",
        "mixer::mixer::scale_f64",
        "execution::program::const_f64 0.5",
        "mixer::mixer::sum_f64 2",
        "execution::program::ret 1",
    ])?;
    let options = VerifyOptions::default().with_type_maps(true);
    let (verified, verification) = Verified::verify(good, options)?;
    assert!(verification.warnings.is_empty());
    assert_eq!(verified.code.len(), 6);

    // Type maps hold the converged state at each block's first instruction.
    let maps = verification.type_maps.expect("requested");
    assert_eq!(maps.get(0, 0).unwrap().operands(), &[] as &[TypeSlot]);

    // `scale_f64` needs two `f64` samples, but finds a `u64` below the top.
    let bad = load(&[
        "execution::program::const_u64 3",
        "execution::program::const_f64 2.0",
        "mixer::mixer::scale_f64",
        "execution::program::ret 1",
    ])?;
    let failed = Verified::verify(bad, options).unwrap_err();
    // `display_named` shows every diagnostic with the module's names.
    assert_eq!(
        failed.display_named().to_string(),
        "verification failed with 1 error\n\
         error: operand 0 of `mixer::mixer::scale_f64`: expected f64, found u64\n  \
         --> @main+2 (address 2): mixer::mixer::scale_f64"
    );
    // It also holds the data: the module, given back, and each diagnostic.
    let (module, rejection) = failed.into_parts();
    let [error] = rejection.diagnostics.as_slice() else { panic!("{rejection}") };
    assert_eq!((error.function, error.address), (0, 2));
    assert_eq!(
        error.kind,
        DiagnosticKind::TypeMismatch {
            role: Some(Role::Operand(0)),
            expected: Type::F64.into(),
            found: TypeSlot::Known(Type::U64),
        }
    );
    assert_eq!(module.code.len(), 4);
    // Rejections carry type maps too, so a tool can show types next to errors.
    assert!(rejection.type_maps.is_some());
    Ok(())
}
```

### Diagnostics

A `Diagnostic` holds data, not text: the function (its index in the function
table), the instruction address, a severity, and a `DiagnosticKind`. Kinds carry
types as `Type` and `TypeSlot` values and table indices, so tools can inspect
them:

- `TypeMismatch { role, expected, found }`: `role` says what the value is to
  the instruction (`Role::Operand(n)`, counted bottom first like a stack
  effect, `Local(i)`, `StoredValue(i)`, `Argument { callee, index }`,
  `Return(n)`, or `Field { struct_id, field }`); `found` is a
  `TypeSlot::Conflict` with the join address when two paths disagreed.
- `StackUnderflow { needed, depth }`, and `StackDepthMismatch` with the
  `Predecessor` edge of each depth (the function entry, a jump, or falling
  through).
- `CallArityMismatch`, `ReturnCountMismatch`, `LocalIndexOutOfRange`,
  `InvalidControlFlowTarget` (with the function's range), `InvalidFunctionSpan`,
  `InvalidFrame`, `MissingTerminator`, and `Unverifiable`.
- Lookups that miss: `UnknownEntry { table, index, len }`, `UnknownField`,
  `UndefinedNamedType`, `NoFunctionAt`, and `FieldCountMismatch`.
- The warning `Unreachable`, and `Rule` for any other error a rule returns.

`diagnostic.display_with(&module)` renders one with the module's names: the
location as `@main+2 (address 2)` followed by the instruction in source syntax,
structs and named types by name, parameters and fields by name. A few examples:

```text
error: argument 0 (p0) of @helper: expected u64, found f64
  --> @main+1 (address 1): execution::program::call 1, @helper
error: local 0 is declared f64, but `execution::program::load_u64` accesses it as u64
  --> @main+0 (address 0): execution::program::load_u64 0
  = help: use `load_f64 0` or `load 0`
error: operand stack depth differs where paths join: 1 from @main+3 (execution::program::br 6), 2 from @main+5 (falling through)
  --> @main+6 (address 6): execution::program::ret 1
error: operand 0 of `cpu::cpu::add_u64`: expected u64, found u64 or f64 (paths disagree at join @main+5)
  --> @main+6 (address 6): cpu::cpu::add_u64
```

`rejection.display_with(&module)` renders a whole rejection, and
`failed.display_named()` does the same for a `VerifyFailed`. A diagnostic's plain `Display` works without the module and
shows indices instead (`struct 0`, `function 0, address 2`).

The instruction text comes from `InstructionText`, which every `#[composite]`
instruction set implements from its patterns (`set_field @Point.y`,
`call 1, @helper`). Only rendering with names needs it: verifying, building
images and running them put no `Display` or `InstructionText` bound on your
instructions. A hand-written instruction set implements `InstructionText` to
get instruction text in diagnostics; at runtime a `StepError` takes its text
from `Dispatch::instruction_text`, which composites generate and hand-written
machines may leave out (the error then shows the location alone). Metadata
validation (`ProgramMetadata::validate`, `into_module`) does format types in
its messages, by the metadata's names, so a custom type model there must be
`DisplayNamed + Clone`; `lower_functions_with_metadata` also needs the header
type to resolve into it (`S: ResolveType<Ty>`).

A failing rule ends its basic block, so a block reports at most one rule error
and code reachable only through it isn't analysed. A block that fails is
checked against the paths that don't run through another failing block, so a
diagnostic never stems from a state some other defect produced. Each defect is
reported once, even inside a loop.

### Type maps

With `VerifyOptions::with_type_maps(true)`, both a `Verification` and a
`Rejection` carry `TypeMaps`: the converged state (locals and operand stack) at
the first instruction of each reached block. `maps.state_at(&module.code, &mut
cx, function, address)` replays a block from its leader to give the state
before any instruction, which is what a debugger or observer needs to show what
each stack slot holds at a breakpoint (pass a `ModuleVerifier::new(&module)` as
`cx`). In a rejection, blocks that were only reachable through a failing block
have no entry.

### Without a `Verified` wrapper

`verify_module(&module, options)` verifies a borrowed module without consuming
it, which is handy in tests. `ModuleVerifier` is the standard context behind
both: it reads the module's function, struct, named-type, constant, and string
tables.

A hand-written context, or a module model the standard verifier doesn't cover,
verifies through `Verified::verify_with(module, |module| ...)`: the closure
builds the context from the module and runs it, and the module is marked
verified when it returns a `Verification`.

`Verified::assume_verified(module)` skips verification explicitly. Use it for
tests that exercise runtime checks with deliberately ill-typed programs, and for
modules a trusted toolchain has already verified. Keep it easy to find: it is
the one place an unverified program can reach an image, outside the
`typed-declarations`-only build described above. (The low-level runner,
`vihaco::execution_state::{execute, resume}`, runs a plain module without an
image and without verification; reach for it only for trusted code.)

## Writing typing rules

A rule implements `Verify<C>` for an instruction type. `C` is the verifier
context, and the rule names only the **capabilities** it needs as bounds, never
a concrete verifier. That keeps rules usable in any machine. A missing rule or
capability is a compile error that says so: "instruction `Pulse` has no
verification rule for context `ModuleVerifier<'m>`", or "verifier context
`FrameState<TypeSlot>` does not provide the `Signatures` capability". In a
`#[composite]`, the headline names the leaf instruction, not the generated
instruction set; rustc's "other types implement" list still shows the
generated `__vihaco_…Instruction` aggregate, which you can ignore.

```rust,ignore
pub trait Verify<C: ?Sized> {
    /// Where control goes after this instruction; `Flow::Next` by default.
    fn flow(&self) -> Flow { Flow::Next }
    /// Check the inputs against the abstract state in `cx` and update it.
    fn verify(&self, cx: &mut C) -> eyre::Result<()>;
}
```

Only control-flow instructions override `flow`: `Flow::jump(target)` for a
branch, `Flow::jump_to([a, b])` for a conditional one, `Flow::Return` and
`Flow::Halt` for terminators. `flow` must depend only on the instruction's own
operands, because the verifier uses it to find blocks before any state exists.

Most rules need only a typed operand stack: bound `C: TypedStack`. Rules with a
fixed stack effect are one line each with `stack_rules!`, as `scale_f64` above;
write `Instruction as this: [..] -> [..];` when the types depend on the
instruction's operands (`array_get T` is
`Get as this: [Type::array(this.0.clone()), Type::U64] -> [this.0.clone()]`).

The standard capabilities, all provided by `ModuleVerifier`:

| Capability | Provides |
|---|---|
| `OperandStack` | `pop`, `pop_typed`, `pop_n`, `push`, `peek`, `depth`; `TypedStack` when its slots are `TypedSlot`s |
| `Locals` | the function's declared locals, `local(i)` and `set_local(i, slot)` |
| `Signatures<Type>` | function signatures by index or address, and the current function's |
| `StructTypes<Type>` | field counts and field types from the struct table |
| `NamedTypes<Type>` | named-type indices by name, and their representations |
| `Constants<Value, Type>` | the constant and string tables |

`stack_effect(cx, [inputs], [outputs])` applies a fixed effect by hand, and
`slot.check(&ty)` checks one slot. Errors from either carry a `Role` when the
helper knows it; add one to your own with `error.with_role(Role::Operand(n))`,
and a fix with `error.with_help("open the channel first")`. Context you wrap
around a `VerifyError` with `eyre::WrapErr::wrap_err` is kept: it renders as
`= note:` lines under the location, and the diagnostic's kind stays
structured.
For instructions several dialects share there are helpers that keep rules
identical: `call_effect` and `return_effect`, `direct_call_effect` and
`indirect_call_effect`, `load_local` and `store_local`, `struct_new_effect` and
`set_field_effect`, `string_constant`, `function_constant`, and `pop_array`. A
call-like instruction must use `call_effect`, `direct_call_effect` or
`indirect_call_effect`, or call `Signatures::after_call` after its operand
effect, so component state is reset after the call (see
[Component state](#component-state)).

A conflicting slot, left where two paths disagreed about a type, never
satisfies a typed pop. Named types are nominal: `type 0` satisfies only itself.

### Counts must be immediates

The verifier only sees the program text, so an instruction's stack effect must
follow from its operands. Carry counts and shapes as immediate operands, as
`sum_f64 2` does above. When an instruction takes its operand count from a
runtime value (say, a count popped from the stack), its rule should report it
with `VerifyError::unverifiable(reason)`, which produces the standard
`Unverifiable` error rather than an ad-hoc message:

```rust
use vihaco::{
    Type, Verify,
    verifier::{TypedStack, VerifyError},
};

/// Pops a `u64` count, then that many samples: not statically checkable.
pub struct SumDynamic;

impl<C: TypedStack + ?Sized> Verify<C> for SumDynamic {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        cx.pop_typed(&Type::U64)?;
        Err(VerifyError::unverifiable("`sum_dynamic` pops its sample count from the stack").into())
    }
}
```

### Component state

Some correctness facts aren't about operand types: a sequencer may need a
channel opened before it plays on it. Such a component defines its own
capability trait and abstract state. The state implements `ComponentState`,
whose `join` merges the states of two paths (a channel is open after a join
only if it is open on every path) and must have finite height. The framework's
`WithState<'_, S>` context wraps `ModuleVerifier` and joins `S` alongside the
frame.

The component crate implements its capability for every `WithState` whose
state *contains* its own (`S: ComponentStateOf<Open>`), not for one concrete
state, so machines can combine it with other components' states:

```rust
use std::collections::BTreeSet;
use vihaco::{
    Verify,
    verifier::{ComponentState, ComponentStateOf, JoinPoint, VerifyError, WithState},
};

/// The sequencer's capability: channels must be opened before use.
pub trait Channels {
    fn open(&mut self, channel: u32);
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
}

#[derive(Clone, Default)]
pub struct Open(BTreeSet<u32>);

impl ComponentState for Open {
    fn join(&mut self, incoming: &Self, _: &JoinPoint) -> Result<bool, VerifyError> {
        let before = self.0.len();
        self.0.retain(|channel| incoming.0.contains(channel));
        Ok(self.0.len() != before)
    }

    /// Nothing is assumed open after a call, the same as at a function entry.
    fn after_call(&mut self, _callee: Option<u32>, entry: &Self) {
        self.clone_from(entry);
    }
}

impl<S: ComponentStateOf<Open>> Channels for WithState<'_, S> {
    fn open(&mut self, channel: u32) {
        self.component_mut::<Open>().0.insert(channel);
    }
    fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
        if self.component::<Open>().0.contains(&channel) {
            Ok(())
        } else {
            Err(VerifyError::rule(format!("channel {channel} is not open")))
        }
    }
}

/// `play c`: the rule needs only the `Channels` capability.
pub struct Play(pub u32);

impl<C: Channels + ?Sized> Verify<C> for Play {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        Ok(cx.require_open(self.0)?)
    }
}
```

An embedder then verifies with `Verified::verify_with_state(module,
Open::default(), options)`, which starts every function with that state.

A machine whose components each add state, possibly from different crates,
declares one combined state with `component_state!`. It joins field by field and
implements `ComponentStateOf` for each field, so every component's capability
works unchanged and the machine crate writes no delegation:

```rust,ignore
vihaco::verifier::component_state! {
    #[derive(Clone, Default)]
    pub struct MachineState {
        pub open: sequencer::Open,
        pub elapsed: timing::Elapsed,
    }
}

let (verified, _) = Verified::verify_with_state(module, MachineState::default(), options)?;
```

Component state does not flow through calls. Each function is verified
separately from the entry state, so the entry state must assume nothing a
caller might not have set up (here: no channels open). After a call, the
caller's state becomes `ComponentState::after_call`, which every state must
define: the callee may have changed anything, so the result must assume no
more than any callee could leave behind. `Open` resets to the entry state:

```text
open 1
call 0, @helper   // the caller's state is reset: channel 1 is no longer open
play 1            // rejected: channel 1 is not open
```

A helper therefore cannot rely on a channel its caller opened; open channels
in the function that plays on them, or after the call.

Resetting to the entry state is right for `Open` because "no channels open"
is safe whatever the callee did. It is wrong for a state that accumulates,
such as the time elapsed against a deadline: its entry state is "0 elapsed",
but a callee may wait any amount, so resetting would accept
`wait 2; call 0, @helper; deadline 10` even when the helper waits 50. Such a
state must become "unknown" after a call. That is why `after_call` has no
default: there is no answer that is sound for every state.

The call helpers, `call_effect` and `indirect_call_effect`, apply
`after_call` for you. A custom call-like rule must use them, or call
`Signatures::after_call` itself after its operand effect. Per-callee
summaries are a possible future extension.

Next, [give your instructions a source spelling](/guide/parser).
