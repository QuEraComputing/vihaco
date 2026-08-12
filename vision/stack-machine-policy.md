# Stack Machine Policy

The stack machine makes the state-ownership rule concrete. Containing a stack does not grant every
instruction direct push and pop access. Stack-owned operations mutate the stack; operations owned
elsewhere cross the composite through messages and effects:

> Native stack operations mutate their selected stack directly. Operations owned by another domain
> obtain stack inputs through message resolution and return stack outputs through effect handling.

`V` is selected by the machine author. It may be a scalar, library newtype, or author-defined
heterogeneous carrier; the stack does not depend on a vihaco `Value` enum. The same policy applies
to frame storage, heaps, and channels. Compatible routes share `V` or another exact Rust boundary
type, while conversions use explicit instructions or handlers as defined in
[`types-and-values.md`](./types-and-values.md).

## Native Stack Instructions

Operations whose semantics are entirely stack-local naturally target the stack component:

- Push an immediate value.
- Drop a value.
- Duplicate a value.
- Swap or rotate values.
- Perform an invariant-preserving stack-local load/store if the stack owns those slots.

The composite still selects each operation explicitly. A method on `Stack<V>` does not become a
program instruction by existing.

## Arithmetic

Reusable arithmetic receives values rather than access to their storage:

```text
resolve:
    consume lhs and rhs from operand_stack

execute on arithmetic component:
    produce lhs + rhs

handle:
    push the result onto operand_stack
```

A fused `stack::Add` remains a valid architecture-specific operation:

- It is simpler and may be faster.
- It can preserve stack-specific atomicity.
- It is less reusable in register or expression-tree machines.
- Its internal stack mutation is less visible to diagnostic handlers.

Both forms may coexist under distinct names or modules. Their difference is architectural rather
than ergonomic: one isolates arithmetic semantics, while the other owns an entire stack
transition.

For a heterogeneous `Stack<MachineValue>`, runtime message resolution may extract and validate
typed operands such as `Operands<i64>` before calling the arithmetic component. Effect handling
then wraps `ValueResult<i64>` back into the author-defined carrier. The reusable arithmetic
component need not know the stack representation.

## Locals and Loads

Separate local and operand storage uses the staged path:

```text
resolve:
    read or consume the local value from locals

execute:
    validate or transform the value if needed

handle:
    push the result to operand_stack
```

When one stack component owns both operand and local-frame semantics, a component-local `Load` may
mutate it directly. State ownership, rather than the instruction's spelling, determines the route.

## Heap Allocation

Heap allocation crosses component boundaries because the heap owns allocation while the stack owns
its operands and result:

```text
resolve:
    consume N values from operand_stack

execute on heap:
    allocate values and produce HeapReference

handle:
    push HeapReference onto operand_stack
```

The heap preserves allocation invariants, and the composite preserves the machine's dataflow.

## Printing

Printing separates value acquisition from external output:

```text
resolve:
    read or consume the selected value
    format or resolve strings according to machine policy

execute:
    produce PrintEffect with owned text

handle:
    deliver to stdout, a diagnostic handler, or a scheduled I/O resource
```

The route resolver determines whether printing reads or consumes the stack value.

## Calls and Control Flow

Control flow emits nominal effects such as `JumpTo`, `Invoke`, and `ReturnFromCall`. The configured
program-counter placement determines their destination:

- With a machine-owned program counter, a route handler applies control effects to the selected
  program-counter component.
- The reference CPUs use this machine-owned arrangement; the root event loop does not independently
  advance their cursors.
- Call-stack selection, frame construction, and return-value placement remain composite routing
  concerns because they cross component boundaries.
- Timing handlers produce owned scheduling information, while the root runtime and `GlobalClock`
  select the next runnable child.

This keeps control-flow instructions reusable across different program, cursor, and frame
representations without allowing both the child and root runtime to advance the same cursor. A
future externally owned cursor can be added after a concrete runtime requires it; the initial
architecture does not generalize that placement.
