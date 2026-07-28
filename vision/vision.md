## Objective

As we begin building on top of vihaco and use it as a compilation target:

1. We should strive to take advantage of Rust's high level features. The goal is to take common vihaco ideas currently supported by macros - messages, effects, instructions, effect observers, etc. - and move them into the type system where possible. This will allow DSL output 

### Composite DSL

### Instruction Set DSL

### Dispatch Loop

- vihaco needs to own 

### Capability Traits

- include justification for the idea of capability traits with examples
    - instruction has two capabilities that perform on a stack; some use cases might use the same
      stack, different stack, etc.

      I might want:
      ```rust
      struct LoadContext<'a> {
        get: &'a Stack,
        push: &'a mut Stack
      }
      ```

      but Rust won't let me use the same stack for get and push because of mutable borrow rules; we
      need to allow for same stack, different stack, etc. capability traits let us do that
- abstraction across runtimes, using multiple contexts for the same instruction, 
  allowing single runtime to impl same trait many times, etc.

### First Class `vihaco` Traits

Instructions

```rust
/// A single instruction.
/// 
/// An instruction receives its [`State`] as a type parameter with capabilities
/// declared. Each capability describes some action that the instruction requires
/// from the external environment. 
trait Instruction<S: State> {
    /// The information needed by the instruction that it doesn't have inline.
    type Message: Message;

    /// The information that exits an instruction and is dispatched to its
    /// handler.
    type Effect: Effect;

    fn execute(
        &self, 
        ctx: &mut S, 
        msg: Self::Message
    ) -> Result<Effects<Self::Effect>, S::Error>;
}
```

Message Resolution

```rust
trait ResolveMessage<I>: State + Sized
where
    I: Instruction<Self>,
{
    fn resolve(&mut self, inst: &I) -> Result<I::Message, Self::Error>;
}

impl<T, I> ResolveMessage<I> for T
where
    T: State + Sized,
    I: Instruction<T, Message = NoMessage>,
{
    #[inline(always)]
    fn resolve(&mut self, _inst: &I) -> Result<I::Message, Self::Error> {
        Ok(NoMessage)
    }
}
```

Effect Handling
```rust
trait ResolveMessage<I>: State + Sized
where
    I: Instruction<Self>,
{
    fn resolve(&mut self, inst: &I) -> Result<I::Message, Self::Error>;
}

impl<T, I> ResolveMessage<I> for T
where
    T: State + Sized,
    I: Instruction<T, Message = NoMessage>,
{
    #[inline(always)]
    fn resolve(&mut self, _inst: &I) -> Result<I::Message, Self::Error> {
        Ok(NoMessage)
    }
}
``## Instructions

We are going to replace the grouping of instruction sets by enums into individual `impl Instruction`
on Rust structs:

```rust
trait Instruction {
    type Message;
    type Result;
    type Fault;

    fn execute(
        &self, 
        msg: Self::Message
    ) -> Result<Self::Result, Self::Fault>;
}
```

We will continue with the idea of an instruction having three stages:

1. **Message Resolution**: What does this instruction need from its execution information?
2. **Instruction Execution**: How does the instruction execute?
3. **Effect Handling**: What effect does this instruction have on its environment?

Moving instructions into their own individual instructions allows for vihaco to know, statically,
what information moves in and out of each instruction. If we were to have a single execute
instruction that matches over variants of an enum, we can construct impossible combinations of
messages and instructions. By moving vihaco ideas into Rust's type system, we can statically ensure
that the information an instruction has during its execution is correct *by construction*.

### Instruction Stepping

The barebones representation of a single instruction's entire pipeline is modeled below:

```rust
fn step<I, S>(instruction: &I, state: &mut S) -> Result<Execution, S::Error>
where 
    I: Instruction,
    S: ResolveMessage<I> + Handle<I::Result>,
    S::Error: From<I::Fault>,
{
    let msg = state.resolve(instruction)?;
    let result = instruction.execute(msg)?;
    state.handle(result)
}
```

This matches exactly with the three stages of an instruction.

### Message Resolution

Message resolution for a specific instruction is dictated through a trait that the machine
implements. This way, instructions stay the same across machines, but the way the message
is resolved can vary based on the encompassing runtime.

```rust
trait ResolveMessage<I>: State + Sized
where
    I: Instruction,
{
    fn resolve(&mut self, inst: &I) -> Result<I::Message, Self::Error>;
}
```

Not all instructions require instructions, so we provide a blanket implementation for
`ResolveMessage` for `Instruction<Message = NoMessage>`:

```rust
impl<T, I> ResolveMessage<I> for T
where
    T: State + Sized,
    I: Instruction<Message = NoMessage>,
{
    #[inline(always)]
    fn resolve(&mut self, _inst: &I) -> Result<I::Message, Self::Error> {
        Ok(NoMessage)
    }
}
```

Message resolution is provided for instructions that need more information from their runtime
environment before they can execute. Think of a `Print` instruction:

```rust
struct Print {
    string: usize,
}
```

The `Print` instruction might require that strings are interned by the loader before program execution
during module resolution, meaning that it only has a `usize` index into a string intern table.

### Support for Asynchronous Instructions

We are going to enforce **instruction boundary suspension**, meaning that an instruction can only perform
a suspending operation in tail position. In vihaco, this will come in the form of an effect, as `Instruction`
requires that `execute` is synchronous. This comes as a 

Take a hypothetical `recv` example:

```rust
struct Receive {
    channel: ChannelId,
}

impl Instruction for Receive {
    /* associated types */

    fn execute(
        &self, 
        msg: Self::Message
    ) -> Result<Self::Result, Self::Fault> {
        /* execution body */
    }
}
```


```rust
enum Execution {
    Complete,
    Parked,
}
```

```rust
fn step<I, S>(instruction: &I, state: &mut S) -> Result<Execution, S::Error>
where 
    I: Instruction,
    S: ResolveMessage<I> + Handle<I::Result>,
    S::Error: From<I::Fault>,
{
    let msg = state.resolve(instruction)?;
    let result = instruction.execute(msg)?;
    state.handle(result)
}
```

---
`
