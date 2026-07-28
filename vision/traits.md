# Vihaco Traits

## Objective

We need to make vihaco ideas - instructions, messages, message resolution, effects, and effect handlers -
first class Rust traits and provide the supporting scaffolding for moving between each stage in the vihaco
framework. 

## Instructions

### Introduction

Currently, instruction sets are defined using a single Rust enum. This works well for vihaco currently, but becomes
limiting when we think about the framework as a) providing composable and reusable components, and b) becoming a compilation
target for composite DSLs:

1. We lose information about the specific messages an instruction will need and what effect(s) an instruction will
   emit;
2. We don't know the state an instruction will need to access or mutate from its execution environment;
3. Instructions are locked into a specific instruction set, and instructions with identical logic will need to be
   implemented twice.

We will introduce a new instruction trait:

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

This will solve the above problems by:

1. Requiring that instructions declare their message and effect as associated types;
2. Requiring the instruction to declare the necessary state and capabilities it needs;
3. Making instructions individual structs that can `impl Instruction` while still being grouped
   by an instruction set enum for cheap dispatch.

### Defining an Instruction Set

We will make use of an `instruction_set!` proc macro 
