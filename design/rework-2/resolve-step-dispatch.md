# Resolution, Step, and generated Dispatch

Status: proposed contracts from the discussion on 2026-10-07. Rust snippets are
API sketches, not implemented interfaces. The execution lifecycle and generic
driver capability bounds still require a concrete implementation proof.

This document isolates source resolution and execution dispatch from the route
builder design. Users can implement Step behavior by hand first. Future typed
route invocations must fit the same execution contract without changing
instruction dispatch or coupling Step to a particular profile.

It refines the earlier synchronous Step examples in
[Composite instruction sets, field markers, and routes](composite-dispatch-and-step.md).
The route discussion remains in [Typed routes and profile-specific drivers](routes-and-drivers.md);
its earlier builder spellings are exploratory. No builder implementation is
required to establish the contracts described here.

## Generated aggregates and authored leaf behavior

`#[composite]` generates one flat in-memory Rust instruction enum containing
the complete available leaf instructions, with distinct variants for distinct
destination field paths. Users do not author `#[instruction_set]` enums or
need to match generated variants to implement resolution and execution.

Bytecode still encodes the nested device-code chain, dialect ID, instruction
variant within the dialect, and payload. Flattening the Rust representation
does not flatten bytecode identity.

Each field has its own type identity, exposed through `field!` in type
position. Two fields containing the same component type remain distinct.
Field markers key resolution and Step implementations; device codes belong
to encoding. Aliases do not create new identities.

For nested devices, generated associations must preserve the complete
destination identity. The exact nested `field!` spelling remains open.
Independently loadable sections use their own resolution and execution
contexts; parent traversal must respect section boundaries.

## Source resolution is separate from execution messages

Source resolution lowers parsed operands, names, and symbols into runtime
instruction leaves. It runs while constructing a program, before execution.

`BuildMessage` supplies runtime input to `Execute` and belongs exclusively to
`ExecuteAt`. It can use runtime operands or a prior awaited completion. It
does not replace source resolution.

The generated surface aggregate selects a leaf resolver; the resolver returns
an authored runtime leaf; generated code wraps it in the flat runtime enum.
Users need neither aggregate enum's generated variant names.

## Field-keyed leaf resolution

An illustrative contract is:

```rust,ignore
trait ResolveInstruction<F, I>
where
    I: InstructionSpec,
{
    type Context;

    fn resolve_instruction(
        &mut self,
        source: I::Surface,
        context: &mut Self::Context,
    ) -> eyre::Result<I>;
}
```

`I` is the runtime instruction type; `I::Surface` is its declared parsed
representation. `F` identifies its destination. The resolver owns the
application's lowering logic, while `Context` provides the relevant symbol
tables, section information, or configuration.

For example, using illustrative future CPU leaf types:

```rust,ignore
impl ResolveInstruction<field!(Stream::cpu), cpu::Branch> for StreamResolver {
    type Context = FunctionSymbols;

    fn resolve_instruction(
        &mut self,
        source: cpu::syntax::Branch,
        symbols: &mut FunctionSymbols,
    ) -> eyre::Result<cpu::Branch> {
        Ok(cpu::Branch(symbols.label_address(&source.target)?))
    }
}
```

The field association allows the same instruction type to resolve against
different program sections or device configuration. Shared conversions can
use ordinary helpers; field-keyed resolution does not require duplicating
their logic.

Generated resolution dispatch performs:

```text
parsed aggregate variant
→ destination field marker + runtime leaf type
→ ResolveInstruction<Field, Leaf>
→ runtime leaf
→ generated flat runtime variant
```

The framework must define how the outer module resolver supplies each leaf's
context. A uniform context or a typed context projection are possible designs;
the associated type above does not itself solve context selection.

## Module lowering remains an outer operation

The existing module-level `Resolve` responsibility remains distinct from the
new leaf hook. Module lowering owns symbol collection, function construction,
address calculation, section metadata, and explicit expansion rules.

Hydra's CPU lowering demonstrates why the leaf hook is not the whole API:
labels disappear from executable code, while branches and calls resolve names
to addresses. Other syntax may expand into multiple instructions.

The one-to-one leaf signature above handles executable leaf conversion.
Non-executable declarations and expansion need separate module-lowering
contracts. A typed emitter could support explicit expansion without exposing
aggregate variants:

```rust,ignore
emitter.emit::<field!(Stream::cpu), cpu::Branch>(resolved_branch)?;
```

This emitter is a proposal. Expansion, address calculation, and resolution
passes must agree on final emitted instruction positions. There is no implicit
surface-to-runtime conversion or assumption that every parsed item emits one
instruction.

## Step constructs an owned execution

Step's result must support more than immediate instruction completion. It
represents owned execution that can run synchronously, submit work, suspend,
and continue after completion.

```rust,ignore
pub struct StepInput<I> {
    pub instruction: I,
    pub execution: ExecutionId,
}

pub trait Step<F, I>: Sized {
    type Execution: Execution<Self>;

    fn step(
        &mut self,
        input: StepInput<I>,
    ) -> eyre::Result<Self::Execution>;
}
```

`ExecutionId` identifies the originating logical execution, not an OS thread.
It allows deferred work to remain associated with the correct execution. Step
inputs do not contain event queues, channel senders, thread handles, or a
selected profile.

Step constructs the owned execution; starting it is a separate runtime action.
Profile-controlled submission and waiting happen through its driver interface,
not while constructing it. Step can inspect logical machine state, but must
not return an execution that retains its temporary mutable machine borrow.

Initially, a user supplies an ordinary handwritten type:

```rust,ignore
impl Step<field!(Stream::signal), signal::Recv> for Stream {
    type Execution = ReceiveExecution;

    fn step(
        &mut self,
        input: StepInput<signal::Recv>,
    ) -> eyre::Result<Self::Execution> {
        Ok(ReceiveExecution {
            execution: input.execution,
            instruction: input.instruction,
        })
    }
}
```

`ReceiveExecution` owns the instruction and execution identity. Its execution
method submits a signal wait with an owned continuation. The continuation
later supplies the edge to instruction execution and applies the result.

## Execution and driver lifecycle

An illustrative execution interface is:

```rust,ignore
pub trait Execution<M>: Sized {
    type Outcome;

    fn start<D>(
        self,
        machine: &mut M,
        driver: &mut D,
    ) -> eyre::Result<()>
    where
        D: Driver<M> + ExecutionControl<Self::Outcome>;
}
```

`Outcome` describes the logical result of the entire execution, such as
continue, halt, or debugger pause. It is not the instruction component's
effect value. A machine's dispatched executions should agree on a logical
outcome type; a generated aggregate can then delegate execution uniformly.

The shared submission contract is:

```rust,ignore
trait Driver<M> {
    fn submit<R, K>(
        &mut self,
        request: R,
        continuation: K,
    ) -> eyre::Result<()>
    where
        R: Request<M>,
        K: Continuation<M, R::Completion>;
}
```

A request describes a logical operation and has a typed completion. A
continuation owns the remainder of execution and accepts that completion.
The profile implements how submission, waiting, and eventual resumption happen.

Returning `Ok(())` from `start` or a continuation means that segment returned
control successfully. It does not imply the logical execution finished: work
and its continuation may now belong to the driver.

Terminal outcomes therefore need an explicit operation. One possible companion
interface is:

```rust,ignore
trait ExecutionControl<O> {
    fn complete(
        &mut self,
        execution: ExecutionId,
        result: eyre::Result<O>,
    ) -> eyre::Result<()>;
}
```

The exact lifecycle interface is not frozen. Its required semantics are:

- The runtime retains the origin identity across every handoff and resumption.
- A suspended execution is not made runnable merely because a Rust call
  returned successfully.
- Terminal completion is reported once, with a logical outcome or failure.
- Submission, deferred-operation, and continuation failures all reach the
  originating execution; a failed operation does not silently drop its wait.
- The caller handles an error returned before a successful handoff. Driver
  implementations handle failures after taking ownership. The final protocol
  must prevent duplicate completion across those paths.

The `Request`, `Continuation`, and driver capability bounds need a concrete
proof: the generic submission sketch does not by itself establish which
operations a particular profile supports or how it stores continuations.

## Generated execution Dispatch

Dispatch selects the Step using the runtime variant's destination marker and
leaf type. It preserves the originating execution identity when constructing
the leaf's `StepInput`.

Conceptually:

```text
flat runtime instruction + execution identity
→ Step<Field, Leaf>::step
→ concrete owned execution
→ generated aggregate execution wrapper
→ start with selected profile's driver
```

The Dispatch surface can use associated types rather than exposing generated
enum names:

```rust,ignore
trait Dispatch: Sized {
    type Instruction;
    type Execution: Execution<Self>;

    fn dispatch(
        &mut self,
        input: StepInput<Self::Instruction>,
    ) -> eyre::Result<Self::Execution>;
}
```

`Instruction` associates the generated flat enum. Different Steps can return
different concrete execution types. Generated code wraps them in an execution
enum and implements `Execution` by delegation, with a common outcome type.
Users do not manually enumerate those dispatch variants.

Missing Step coverage should be checked at the executable machine/Dispatch
boundary, with diagnostics identifying the destination field and leaf type.
Declaring available component instructions is separate from providing a
machine's executable behavior.

## Compatibility with future routes and profiles

A future route invocation implements the same `Execution<M>` interface as a
handwritten execution. It carries the instruction, origin identity, and any
saved values. Step and generated Dispatch do not need to change when a user
replaces handwritten execution with a builder implementation.

The agreed instruction-execution builder spelling is:

```rust,ignore
.on::<field!(Stream::signal)>()
.build_message_and_execute::<signal::Recv>()
```

It explicitly configures `ExecuteAt` with `BuildMessage`, then `Execute`.
`Execute` produces its associated effect value, not an `Effects<E>` container.
Subsequent route stages handle that value as ordinary typed information.

At an explicit work boundary, the future builder calls `Driver::submit` with
the incoming request and the owned remaining route. `.then(...)` only
transforms information; `.await_work()` performs submission and waiting and
is followed by `.then(...)` accepting the request's completion.

An awaited completion resumes the continuation, not the original Step. The
continuation must support carrying both the pending instruction and its typed
completion into `ExecuteAt`, where `BuildMessage` uses them. This execution
input contract remains to be designed; it must not require hidden scratch
state on the machine or ad hoc message-building closures.

Routes and handwritten executions describe shared logical information flow,
destinations, and dependencies. Drivers own scheduling correctness, virtual
time, event queues, ready/waiting state, async waits, and worker coordination.
Emulation retains continuations around events or registered waits. Real-time
profiles retain them around endpoint completions. Neither requires different
Step definitions merely because its wait mechanism differs.

No mandatory trait-object execution, boxed continuation, or universal `Send`
bound is imposed by Step. Continuation storage is a driver decision. Values
that actually cross threads need appropriate transfer bounds; locally retained
continuations do not acquire those bounds merely because a profile has workers.
Profile-selected endpoint ownership and permitted machine access still need
to be specified.

## Establishing the contracts before builders

The first implementation should prove these boundaries with handwritten types:

1. Resolve a leaf at a particular field without matching aggregate enums.
2. Dispatch two instances of the same component to distinct Step implementations.
3. Execute an immediate operation and explicitly report its logical outcome.
4. Submit a wait, release the machine borrow, and resume an owned continuation.
5. Resume through a second boundary before reporting final completion.
6. Run the same handwritten behavior with emulation and real-time-style drivers.
7. Propagate failures through submission, completion, and continuation paths.

Resolve context selection, pseudo-instruction lowering, typed execution inputs,
endpoint ownership, and lifecycle capability bounds remain open details. The
architectural requirement is fixed: leaf resolution and Step behavior are
authored against field identities, aggregate dispatch is generated, and Step
returns owned execution that future route builders can construct.
