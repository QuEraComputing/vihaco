# vihaco concepts used by the demo

The demo separates reusable components from the machine-specific runtime that
contains them. Its small contracts mirror the current `vihaco` runtime API;
the larger event loop remains ordinary Rust so the ownership boundaries are
visible.

## Components and products

`component!` declares a component and its owned runtime instruction products:

```rust
component! {
    component Stack { items: Vec<i64>, }
    runtime {
        instruction { Push(i64), Pop, }
    }
}
```

This declaration generates `Stack` and the `Push`/`Pop` product structs. It
does not generate a component-wide instruction enum; the containing composite
chooses which products to expose and owns that sum.

The implementation is per product. The demo's arithmetic unit implements
`Execute<Add>`, `Execute<Sub>`, and `Execute<Mul>` independently. Each
implementation chooses its own `Message`, `Effect`, and `Fault` types.

```rust
impl Execute<Add> for ArithmeticUnit {
    type Message = BinaryOperands;
    type Effect = ValueResult;
    type Fault = ArithmeticFault;

    fn execute(
        &mut self,
        _: &Add,
        operands: BinaryOperands,
    ) -> Result<StepResult<ValueResult>, ArithmeticFault> {
        Ok(StepResult {
            effects: Effects::one(ValueResult(operands.lhs + operands.rhs)),
            execution: Execution::Complete,
        })
    }
}
```

## Composite routes

The demo's CPU uses `composite!` to select the products it exposes and connect
them to capabilities:

```rust
composite! {
    composite Cpu {
        error = CpuFault;
        operand_stack: Stack,
        alu: ArithmeticUnit,
    }

    runtime {
        IntegerAdd(Add) => alu {
            message from operand_stack;
            effects { absorb with operand_stack; }
        }
    }
}
```

The generated `CpuInstruction` is a machine-local sum. A route resolves an
owned message, executes the selected product, observes each effect, and sends
ownership to one handler. `Supply` and `Absorb` keep the reusable stack
independent of this particular CPU.

## Effects and parked work

`Effects<E>` represents zero, one, or many homogeneous effects. `StepResult`
pairs those effects with `Execution::Complete` or `Execution::Parked`.

The demo's receive operation can park. The child owns its continuation and
knows how to resume it; the parent owns the event loop, global clock, endpoint
identity, and scheduling policy. This keeps a reusable channel from knowing
whether it is hosted by one CPU or several.

Timing and continuation dispatch are intentionally hand-written in the current
API. A future extension may generate more of that plumbing; until it exists,
the demo's explicit root loop is the authoritative pattern.
