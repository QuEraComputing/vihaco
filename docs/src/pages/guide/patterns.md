---
layout: ../../layouts/Guide.astro
title: Patterns for larger machines
slug: patterns
description: "Reuse typed input preparation and execution policy without coupling devices to runtimes."
---

# Patterns for larger machines

As you add instructions, you'll often find yourself preparing similar inputs
and handling results the same way. You can share that work without combining
the instructions' individual behavior.

The message and step examples use the same small arithmetic machine. You can find the
[complete runnable source](https://github.com/QuEraComputing/vihaco/blob/main/docs/examples/patterns.rs)
in the repository; it works with the [guide setup](/guide).

## Group by message shape

A dialect library can publish traits in a module such as `message_kind` to
group instructions with the same preparation needs. For example, several
instructions might take two addresses and a numeric parameter. The trait can
also expose accessors for payload fields the builder needs.

On the composite, write one `BuildMessage` impl bounded by the shape trait.
Name the message type explicitly and require it to match the selected child's
`Execute<I>::Message`. The builder can then resolve inputs for every instruction
in the group. Each instruction still has its own execution behavior.

Here's a small example. `Add` and `Subtract` name two addresses in the machine's
`values` table, while `Read` names one. The message-kind traits expose those
addresses so builders can resolve them. `Calculator` executes each operation
with the resolved values, and the composite keeps the results.

```rust
use vihaco::{BuildMessage, Execute, Step, component, composite, dialect, field};

#[dialect]
pub mod arithmetic {
    #[instruction]
    pub struct Add(pub usize, pub usize);

    #[instruction]
    pub struct Subtract(pub usize, pub usize);

    #[instruction]
    pub struct Read(pub usize);
}

pub mod message_kind {
    pub trait Binary {
        fn addresses(&self) -> (usize, usize);
    }

    pub trait Unary {
        fn address(&self) -> usize;
    }
}

impl message_kind::Binary for arithmetic::Add {
    fn addresses(&self) -> (usize, usize) {
        (self.0, self.1)
    }
}

impl message_kind::Binary for arithmetic::Subtract {
    fn addresses(&self) -> (usize, usize) {
        (self.0, self.1)
    }
}

impl message_kind::Unary for arithmetic::Read {
    fn address(&self) -> usize {
        self.0
    }
}

pub struct BinaryInput(pub u64, pub u64);

pub struct UnaryInput(pub u64);
# #[component]
# #[instructions { arithmetic }]
# pub struct Calculator;
#
# impl Execute<arithmetic::Add> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Add, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_add(input.1)
#             .ok_or_else(|| eyre::eyre!("addition overflow"))
#     }
# }
#
# impl Execute<arithmetic::Subtract> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Subtract, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_sub(input.1)
#             .ok_or_else(|| eyre::eyre!("subtraction underflow"))
#     }
# }
#
# impl Execute<arithmetic::Read> for Calculator {
#     type Message = UnaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Read, input: UnaryInput) -> eyre::Result<u64> {
#         Ok(input.0)
#     }
# }

#[composite]
pub struct Machine {
    #[device(1)]
    calculator: Calculator,
    values: Vec<u64>,
    results: Vec<u64>,
}

impl Machine {
    fn value(&self, address: usize) -> eyre::Result<u64> {
        self.values
            .get(address)
            .copied()
            .ok_or_else(|| eyre::eyre!("unknown value address: {address}"))
    }
}

impl<I> BuildMessage<field!(Machine::calculator), I, (), BinaryInput> for Machine
where
    I: message_kind::Binary,
    Calculator: Execute<I, Message = BinaryInput>,
{
    fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<BinaryInput> {
        let (lhs, rhs) = instruction.addresses();
        Ok(BinaryInput(self.value(lhs)?, self.value(rhs)?))
    }
}
# impl<I> BuildMessage<field!(Machine::calculator), I, (), UnaryInput> for Machine
# where
#     I: message_kind::Unary,
#     Calculator: Execute<I, Message = UnaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<UnaryInput> {
#         Ok(UnaryInput(self.value(instruction.address())?))
#     }
# }
# impl<I> Step<field!(Machine::calculator), I> for Machine
# where
#     Calculator: Execute<I, Effect = u64, Error = eyre::Report>,
#     Self: BuildMessage<field!(Machine::calculator), I>,
# {
#     fn step(&mut self, instruction: &I, context: &()) -> eyre::Result<()> {
#         let message = self.build_message(instruction, context)?;
#         let result = self.calculator.execute(instruction, message)?;
#         self.results.push(result);
#         Ok(())
#     }
# }
# pub fn main() -> eyre::Result<()> {
#     let mut machine = Machine {
#         calculator: Calculator,
#         values: vec![20, 22],
#         results: vec![],
#     };
#     type Destination = field!(Machine::calculator);
#
#     <Machine as Step<Destination, arithmetic::Add>>::step(
#         &mut machine,
#         &arithmetic::Add(0, 1),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Subtract>>::step(
#         &mut machine,
#         &arithmetic::Subtract(1, 0),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Read>>::step(
#         &mut machine,
#         &arithmetic::Read(0),
#         &(),
#     )?;
#     assert_eq!(machine.results, vec![42, 2, 20]);
#
#     // An invalid address fails before execution adds another result.
#     assert!(
#         <Machine as Step<Destination, arithmetic::Read>>::step(
#             &mut machine,
#             &arithmetic::Read(99),
#             &(),
#         )
#         .is_err()
#     );
#     assert_eq!(machine.results, vec![42, 2, 20]);
#     Ok(())
# }
```

Both binary operations use this one builder. Their execution implementations
still choose whether to add or subtract, and report overflow or underflow.
The builder reports an error if either address is missing.

If a standard stack message already describes the input, use that. Custom shapes
are useful when preparation also resolves references, validates input, or reads
application-specific data.

## Keep blanket implementations disjoint

Rust doesn't treat different marker bounds as proof that two blanket builders
can't overlap. Even if you keep the groups separate, the coherence checker can
reject the impls.

Name distinct message types explicitly to separate the builder impls, and
require the child's associated message to match. Standard stack messages already
have framework-supplied preparation, so custom builders only need to handle
your own message types.

`MemberOf<G>` helps group types, but doesn't declare execution support or bypass
the overlap rules.

The unary builder names `UnaryInput`, keeping it distinct from the
`BinaryInput` builder above:

```rust
# use vihaco::{BuildMessage, Execute, Step, component, composite, dialect, field};
# #[dialect]
# pub mod arithmetic {
#     #[instruction]
#     pub struct Add(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Subtract(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Read(pub usize);
# }
# pub mod message_kind {
#     pub trait Binary {
#         fn addresses(&self) -> (usize, usize);
#     }
#
#     pub trait Unary {
#         fn address(&self) -> usize;
#     }
# }
#
# impl message_kind::Binary for arithmetic::Add {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Binary for arithmetic::Subtract {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Unary for arithmetic::Read {
#     fn address(&self) -> usize {
#         self.0
#     }
# }
# pub struct BinaryInput(pub u64, pub u64);
#
# pub struct UnaryInput(pub u64);
# #[component]
# #[instructions { arithmetic }]
# pub struct Calculator;
#
# impl Execute<arithmetic::Add> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Add, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_add(input.1)
#             .ok_or_else(|| eyre::eyre!("addition overflow"))
#     }
# }
#
# impl Execute<arithmetic::Subtract> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Subtract, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_sub(input.1)
#             .ok_or_else(|| eyre::eyre!("subtraction underflow"))
#     }
# }
#
# impl Execute<arithmetic::Read> for Calculator {
#     type Message = UnaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Read, input: UnaryInput) -> eyre::Result<u64> {
#         Ok(input.0)
#     }
# }
# #[composite]
# pub struct Machine {
#     #[device(1)]
#     calculator: Calculator,
#     values: Vec<u64>,
#     results: Vec<u64>,
# }
#
# impl Machine {
#     fn value(&self, address: usize) -> eyre::Result<u64> {
#         self.values
#             .get(address)
#             .copied()
#             .ok_or_else(|| eyre::eyre!("unknown value address: {address}"))
#     }
# }
# impl<I> BuildMessage<field!(Machine::calculator), I, (), BinaryInput> for Machine
# where
#     I: message_kind::Binary,
#     Calculator: Execute<I, Message = BinaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<BinaryInput> {
#         let (lhs, rhs) = instruction.addresses();
#         Ok(BinaryInput(self.value(lhs)?, self.value(rhs)?))
#     }
# }
impl<I> BuildMessage<field!(Machine::calculator), I, (), UnaryInput> for Machine
where
    I: message_kind::Unary,
    Calculator: Execute<I, Message = UnaryInput>,
{
    fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<UnaryInput> {
        Ok(UnaryInput(self.value(instruction.address())?))
    }
}
# impl<I> Step<field!(Machine::calculator), I> for Machine
# where
#     Calculator: Execute<I, Effect = u64, Error = eyre::Report>,
#     Self: BuildMessage<field!(Machine::calculator), I>,
# {
#     fn step(&mut self, instruction: &I, context: &()) -> eyre::Result<()> {
#         let message = self.build_message(instruction, context)?;
#         let result = self.calculator.execute(instruction, message)?;
#         self.results.push(result);
#         Ok(())
#     }
# }
# pub fn main() -> eyre::Result<()> {
#     let mut machine = Machine {
#         calculator: Calculator,
#         values: vec![20, 22],
#         results: vec![],
#     };
#     type Destination = field!(Machine::calculator);
#
#     <Machine as Step<Destination, arithmetic::Add>>::step(
#         &mut machine,
#         &arithmetic::Add(0, 1),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Subtract>>::step(
#         &mut machine,
#         &arithmetic::Subtract(1, 0),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Read>>::step(
#         &mut machine,
#         &arithmetic::Read(0),
#         &(),
#     )?;
#     assert_eq!(machine.results, vec![42, 2, 20]);
#
#     // An invalid address fails before execution adds another result.
#     assert!(
#         <Machine as Step<Destination, arithmetic::Read>>::step(
#             &mut machine,
#             &arithmetic::Read(99),
#             &(),
#         )
#         .is_err()
#     );
#     assert_eq!(machine.results, vec![42, 2, 20]);
#     Ok(())
# }
```

Both builders are generic over `I`, but their explicit fourth type parameters
are different: `BinaryInput` and `UnaryInput`. The marker bounds select the input
shape, while those message types keep the impls disjoint. Each equality bound
also checks that the component expects the message being built.

A few clear shape contracts are easier to work with than preparation code that
inspects every instruction at runtime. Test the shared ordering and validation
with representative instructions from each shape.

## Share continuation by destination

One generic `Step` can serve a device when its instructions all follow the same
sequence: prepare input, execute, and handle the result. Its bounds describe
the input and output requirements. Different devices can use different shared
steps, even for the same instruction type.

All three operations return a `u64`, so one step can prepare their different
messages and collect their results:

```rust
# use vihaco::{BuildMessage, Execute, Step, component, composite, dialect, field};
# #[dialect]
# pub mod arithmetic {
#     #[instruction]
#     pub struct Add(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Subtract(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Read(pub usize);
# }
# pub mod message_kind {
#     pub trait Binary {
#         fn addresses(&self) -> (usize, usize);
#     }
#
#     pub trait Unary {
#         fn address(&self) -> usize;
#     }
# }
#
# impl message_kind::Binary for arithmetic::Add {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Binary for arithmetic::Subtract {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Unary for arithmetic::Read {
#     fn address(&self) -> usize {
#         self.0
#     }
# }
# pub struct BinaryInput(pub u64, pub u64);
#
# pub struct UnaryInput(pub u64);
# #[component]
# #[instructions { arithmetic }]
# pub struct Calculator;
#
# impl Execute<arithmetic::Add> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Add, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_add(input.1)
#             .ok_or_else(|| eyre::eyre!("addition overflow"))
#     }
# }
#
# impl Execute<arithmetic::Subtract> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Subtract, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_sub(input.1)
#             .ok_or_else(|| eyre::eyre!("subtraction underflow"))
#     }
# }
#
# impl Execute<arithmetic::Read> for Calculator {
#     type Message = UnaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Read, input: UnaryInput) -> eyre::Result<u64> {
#         Ok(input.0)
#     }
# }
# #[composite]
# pub struct Machine {
#     #[device(1)]
#     calculator: Calculator,
#     values: Vec<u64>,
#     results: Vec<u64>,
# }
#
# impl Machine {
#     fn value(&self, address: usize) -> eyre::Result<u64> {
#         self.values
#             .get(address)
#             .copied()
#             .ok_or_else(|| eyre::eyre!("unknown value address: {address}"))
#     }
# }
# impl<I> BuildMessage<field!(Machine::calculator), I, (), BinaryInput> for Machine
# where
#     I: message_kind::Binary,
#     Calculator: Execute<I, Message = BinaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<BinaryInput> {
#         let (lhs, rhs) = instruction.addresses();
#         Ok(BinaryInput(self.value(lhs)?, self.value(rhs)?))
#     }
# }
# impl<I> BuildMessage<field!(Machine::calculator), I, (), UnaryInput> for Machine
# where
#     I: message_kind::Unary,
#     Calculator: Execute<I, Message = UnaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<UnaryInput> {
#         Ok(UnaryInput(self.value(instruction.address())?))
#     }
# }
impl<I> Step<field!(Machine::calculator), I> for Machine
where
    Calculator: Execute<I, Effect = u64, Error = eyre::Report>,
    Self: BuildMessage<field!(Machine::calculator), I>,
{
    fn step(&mut self, instruction: &I, context: &()) -> eyre::Result<()> {
        let message = self.build_message(instruction, context)?;
        let result = self.calculator.execute(instruction, message)?;
        self.results.push(result);
        Ok(())
    }
}
# pub fn main() -> eyre::Result<()> {
#     let mut machine = Machine {
#         calculator: Calculator,
#         values: vec![20, 22],
#         results: vec![],
#     };
#     type Destination = field!(Machine::calculator);
#
#     <Machine as Step<Destination, arithmetic::Add>>::step(
#         &mut machine,
#         &arithmetic::Add(0, 1),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Subtract>>::step(
#         &mut machine,
#         &arithmetic::Subtract(1, 0),
#         &(),
#     )?;
#     <Machine as Step<Destination, arithmetic::Read>>::step(
#         &mut machine,
#         &arithmetic::Read(0),
#         &(),
#     )?;
#     assert_eq!(machine.results, vec![42, 2, 20]);
#
#     // An invalid address fails before execution adds another result.
#     assert!(
#         <Machine as Step<Destination, arithmetic::Read>>::step(
#             &mut machine,
#             &arithmetic::Read(99),
#             &(),
#         )
#         .is_err()
#     );
#     assert_eq!(machine.results, vec![42, 2, 20]);
#     Ok(())
# }
```

The bounds let Rust pick the right builder for `I`. This step handles the
component's result directly; a machine using `PushToStack` could call `Apply`
instead. A preparation or execution error returns before a new result is added.

If an instruction needs extra behavior, keep that visible. Take care not to
introduce overlapping blanket and specialized step impls.

## Substitute implementations behind one component

A wrapper can forward `Execute<I>` to an interchangeable backend. Give the
backends matching message, effect, and error types for that contract. Then you
can swap the implementation while keeping the composite's preparation and
result handling.

This wrapper forwards instructions to its backend. Its declaration requires
complete support for the counter dialect, with the same message, effect, and
error types for any backend you put inside it:

```rust
use vihaco::{Execute, NoMessage, component, dialect};

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}

#[component]
#[instructions { counter }]
#[derive(Default)]
pub struct Counter(u64);

impl Execute<counter::Add> for Counter {
    type Message = NoMessage;
    type Effect = u64;
    type Error = eyre::Report;

    fn execute(&mut self, instruction: &counter::Add, _: NoMessage) -> eyre::Result<u64> {
        self.0 = self
            .0
            .checked_add(instruction.0)
            .ok_or_else(|| eyre::eyre!("counter overflow"))?;
        Ok(self.0)
    }
}

#[component]
#[instructions { counter }]
pub struct Device<B>
where
    B: Execute<counter::Add, Message = NoMessage, Effect = u64, Error = eyre::Report>,
{
    backend: B,
}

impl<B, I> Execute<I> for Device<B>
where
    B: Execute<counter::Add, Message = NoMessage, Effect = u64, Error = eyre::Report> + Execute<I>,
{
    type Message = <B as Execute<I>>::Message;
    type Effect = <B as Execute<I>>::Effect;
    type Error = <B as Execute<I>>::Error;

    fn execute(
        &mut self,
        instruction: &I,
        message: Self::Message,
    ) -> Result<Self::Effect, Self::Error> {
        self.backend.execute(instruction, message)
    }
}

fn main() -> eyre::Result<()> {
    let mut device = Device {
        backend: Counter::default(),
    };
    assert_eq!(device.execute(&counter::Add(5), NoMessage::new())?, 5);
    assert_eq!(device.execute(&counter::Add(2), NoMessage::new())?, 7);
    Ok(())
}
```

Here `Counter` is the backend. Another backend meeting the same bound can occupy
that field without changing the forwarding method. For a larger dialect, the
wrapper's bounds need to establish support for every declared instruction.

As the code grows, put implementations into modules by instruction family and
backend. Shared message preparation stays with the composite; the computation
stays with the device.

## Give a component its own runner

Some components need host-side work between instructions: a debugger must stop
and resume at breakpoints, while a device driver might wait for an event or
limit how much work happens in one call. Keep that policy in a runner alongside
the component. The component still implements `Execute`, and the composite's
authored `Step` still chooses how to apply its effects.

The debugger's `resume_with_breaks` is one example. Its machine uses
`ExecutionState<DebuggerState>`; applying the debugger's `Break` effect suspends
the invocation. The runner interprets `Continue` or `Step`, selects the next
continuation, and executes a saved instruction when resuming a gutter
breakpoint. Ordinary `resume` leaves a suspended invocation untouched.

You can build your own runner from the public helpers in
`vihaco_stdlib::execution_state`:

| Helper | Responsibility |
| --- | --- |
| `run(machine, initialize, step)` | Reject reentrant execution, initialize per-call driver state, and call `step` while the invocation is runnable. |
| `execute_instruction(module, machine, instruction_override)` | Check and fetch the current instruction, advance the PC, and dispatch it. Restore the PC on an error, halt, or suspension. |

`run` calls `initialize` once, even when the invocation is halted or suspended.
It passes the resulting state to each `step` call. A step returns
`ControlFlow::Continue(())` to keep driving or `ControlFlow::Break(())` to return
to the host without changing the invocation's run state. The execution guard is
released when either closure returns an error; stack, frame, and device
mutations are retained. The guard is not released if a closure panics.

For example, this runner executes at most `budget` instructions per call. It
uses the same checked dispatch as the ordinary runner and leaves the invocation
runnable when the budget is exhausted:

```rust
use std::ops::ControlFlow;
use vihaco::{Dispatch, HasProgram, module::LocalModule};
use vihaco_stdlib::{
    ExecutionState,
    execution_state::{execute_instruction, run},
};
# use vihaco::{BuildMessage, Execute, HasInstructionSet, Step, composite, field};
# use vihaco_stdlib::program::program;
# #[composite]
# struct Machine {
#     #[device(0)] #[program]
#     execution: vihaco_stdlib::ExecutionState,
# }
# type Instruction = <Machine as HasInstructionSet>::Runtime;
# type Program = LocalModule<Instruction, (), ()>;
# impl<I> Step<field!(Machine::execution), I, Program> for Machine
# where
#     ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
#     Self: BuildMessage<field!(Machine::execution), I, Program>,
# {
#     fn step(&mut self, instruction: &I, module: &Program) -> eyre::Result<()> {
#         let message = <Self as BuildMessage<field!(Machine::execution), I, Program>>
#             ::build_message(self, instruction, module)?;
#         self.execution.execute(instruction, message)
#     }
# }

fn resume_for<M, I, V, Ty, Info, S>(
    module: &LocalModule<I, V, Ty, Info>,
    machine: &mut M,
    budget: usize,
) -> eyre::Result<()>
where
    M: HasProgram<Program = ExecutionState<S>>
        + Dispatch<LocalModule<I, V, Ty, Info>, Instruction = I>,
{
    run(
        machine,
        |_| Ok(budget),
        |machine, remaining| {
            if *remaining == 0 {
                return Ok(ControlFlow::Break(()));
            }
            execute_instruction(module, machine, None)?;
            *remaining -= 1;
            Ok(ControlFlow::Continue(()))
        },
    )
}
# fn main() -> eyre::Result<()> {
#     use vihaco::module::{FunctionInfo, Signature};
#     let module = Program {
#         code: vec![
#             Instruction::V_execution__program__ConstU64(program::ConstU64(20)),
#             Instruction::V_execution__program__ConstU64(program::ConstU64(22)),
#             Instruction::V_execution__program__Return(program::Return(2)),
#         ],
#         main_function: Some(0),
#         functions: vec![FunctionInfo {
#             name: 0, signature: Signature { params: vec![], ret: vec![] },
#             local_count: 0, start_address: 0, end_address: 3, file: 0,
#         }],
#         ..Default::default()
#     };
#     let mut machine = Machine { execution: ExecutionState::new() };
#     machine.execution.start(&module, &[])?;
#     resume_for(&module, &mut machine, 1)?;
#     assert_eq!(machine.execution.pc(), 1);
#     assert_eq!(machine.execution.stack(), &[20]);
#     resume_for(&module, &mut machine, 0)?;
#     assert_eq!(machine.execution.pc(), 1);
#     assert_eq!(machine.execution.stack(), &[20]);
#     resume_for(&module, &mut machine, 10)?;
#     assert!(machine.execution.is_halted());
#     assert_eq!(machine.execution.stack(), &[20, 22]);
#     Ok(())
# }
```

Call `ExecutionState::start` before driving a new invocation, and keep supplying
the same module. Pass `None` to `execute_instruction` for ordinary fetching. The
debugger passes a saved instruction as `Some(&instruction)` to execute it once
without removing the installed breakpoint. Even with an override, the PC must
address an instruction in the module.

For a driver that needs suspension, choose a custom `S` for `ExecutionState<S>`.
Its effect handler can call `suspend(state)`; the runner sees `RunState::Other`
and stops. Save the desired continuation in that state before suspending:
`execute_instruction` restores the PC to the current instruction after dispatch
suspends. On the next call, your runner's initializer can inspect `run_state()`
and call `resume_at(pc)` when the driver is ready. `resume_at` selects a PC and
makes the invocation runnable; it does not create frames or validate the PC.
The next `execute_instruction` checks the active frame and fetch position.

The debugger exposes its driver as a free `resume_with_breaks` function and a
`Loadable` convenience method. Library code can also add inherent methods in
`impl ExecutionState<DriverState>` blocks. In an external crate, Rust does not
allow inherent impls on the library's `ExecutionState` or `Loadable`. Publish a
free runner function, implement your own extension trait for those types, or
add a method on an application-owned wrapper. You can expose the driver-specific
entry point without changing the shared execution machinery.

See the [debugger runner source](https://github.com/QuEraComputing/vihaco/blob/main/crates/vihaco-stdlib/src/debugger/runner.rs)
for the breakpoint policy, and run the complete integration example with
`cargo run -p vihaco-stdlib --features program-image-execution --example debugger`.

## Typed interactive execution

A REPL or application command can construct a leaf instruction and call its
step directly. You don't have to append code to a program or inspect the
aggregate enum for that.

You can call the arithmetic machine's shared step directly with typed leaf
instructions:

```rust
# use vihaco::{BuildMessage, Execute, Step, component, composite, dialect, field};
# #[dialect]
# pub mod arithmetic {
#     #[instruction]
#     pub struct Add(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Subtract(pub usize, pub usize);
#
#     #[instruction]
#     pub struct Read(pub usize);
# }
# pub mod message_kind {
#     pub trait Binary {
#         fn addresses(&self) -> (usize, usize);
#     }
#
#     pub trait Unary {
#         fn address(&self) -> usize;
#     }
# }
#
# impl message_kind::Binary for arithmetic::Add {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Binary for arithmetic::Subtract {
#     fn addresses(&self) -> (usize, usize) {
#         (self.0, self.1)
#     }
# }
#
# impl message_kind::Unary for arithmetic::Read {
#     fn address(&self) -> usize {
#         self.0
#     }
# }
# pub struct BinaryInput(pub u64, pub u64);
#
# pub struct UnaryInput(pub u64);
# #[component]
# #[instructions { arithmetic }]
# pub struct Calculator;
#
# impl Execute<arithmetic::Add> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Add, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_add(input.1)
#             .ok_or_else(|| eyre::eyre!("addition overflow"))
#     }
# }
#
# impl Execute<arithmetic::Subtract> for Calculator {
#     type Message = BinaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Subtract, input: BinaryInput) -> eyre::Result<u64> {
#         input
#             .0
#             .checked_sub(input.1)
#             .ok_or_else(|| eyre::eyre!("subtraction underflow"))
#     }
# }
#
# impl Execute<arithmetic::Read> for Calculator {
#     type Message = UnaryInput;
#     type Effect = u64;
#     type Error = eyre::Report;
#
#     fn execute(&mut self, _: &arithmetic::Read, input: UnaryInput) -> eyre::Result<u64> {
#         Ok(input.0)
#     }
# }
# #[composite]
# pub struct Machine {
#     #[device(1)]
#     calculator: Calculator,
#     values: Vec<u64>,
#     results: Vec<u64>,
# }
#
# impl Machine {
#     fn value(&self, address: usize) -> eyre::Result<u64> {
#         self.values
#             .get(address)
#             .copied()
#             .ok_or_else(|| eyre::eyre!("unknown value address: {address}"))
#     }
# }
# impl<I> BuildMessage<field!(Machine::calculator), I, (), BinaryInput> for Machine
# where
#     I: message_kind::Binary,
#     Calculator: Execute<I, Message = BinaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<BinaryInput> {
#         let (lhs, rhs) = instruction.addresses();
#         Ok(BinaryInput(self.value(lhs)?, self.value(rhs)?))
#     }
# }
# impl<I> BuildMessage<field!(Machine::calculator), I, (), UnaryInput> for Machine
# where
#     I: message_kind::Unary,
#     Calculator: Execute<I, Message = UnaryInput>,
# {
#     fn build_message(&mut self, instruction: &I, _: &()) -> eyre::Result<UnaryInput> {
#         Ok(UnaryInput(self.value(instruction.address())?))
#     }
# }
# impl<I> Step<field!(Machine::calculator), I> for Machine
# where
#     Calculator: Execute<I, Effect = u64, Error = eyre::Report>,
#     Self: BuildMessage<field!(Machine::calculator), I>,
# {
#     fn step(&mut self, instruction: &I, context: &()) -> eyre::Result<()> {
#         let message = self.build_message(instruction, context)?;
#         let result = self.calculator.execute(instruction, message)?;
#         self.results.push(result);
#         Ok(())
#     }
# }
pub fn main() -> eyre::Result<()> {
    let mut machine = Machine {
        calculator: Calculator,
        values: vec![20, 22],
        results: vec![],
    };
    type Destination = field!(Machine::calculator);

    <Machine as Step<Destination, arithmetic::Add>>::step(
        &mut machine,
        &arithmetic::Add(0, 1),
        &(),
    )?;
    <Machine as Step<Destination, arithmetic::Subtract>>::step(
        &mut machine,
        &arithmetic::Subtract(1, 0),
        &(),
    )?;
    <Machine as Step<Destination, arithmetic::Read>>::step(
        &mut machine,
        &arithmetic::Read(0),
        &(),
    )?;
    assert_eq!(machine.results, vec![42, 2, 20]);

    // An invalid address fails before execution adds another result.
    assert!(
        <Machine as Step<Destination, arithmetic::Read>>::step(
            &mut machine,
            &arithmetic::Read(99),
            &(),
        )
        .is_err()
    );
    assert_eq!(machine.results, vec![42, 2, 20]);
    Ok(())
}
```

These operations use the machine's value table and a unit context, so they
need no running program. The results are `42`, `2`, and `20`; an unknown address
returns an error without appending a result.

Supply the context and invocation state the step expects. A direct call won't
run the program runner's setup or manage instruction progression.

[Testing and extending](/guide/testing) covers how to check these pieces and add
new ones.
