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
