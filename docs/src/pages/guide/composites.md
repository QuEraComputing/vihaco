---
layout: ../../layouts/Guide.astro
title: Defining a Composite
slug: composites
description: "Composite structs are the composition root in vihaco — #[composite] generates the outer instruction enum and device metadata; message resolution and effect delivery are hand-written."
---

# Defining A Composite With `vihaco`

Composite structs are the composition root in `vihaco`.
They own components, observers, and device codes, and they are where the
generated wiring meets your hand-written runtime.

This guide shows how to wire components and observers into a composite using the current macro surface.

If you have not read the observer guide yet, read [Observing Effects With `#[observe]`](/guide/observers) first.
For a focused guide to composite-side message resolution before component execution, read [Using Messages With `vihaco`](/guide/messages).

## A Small Composite

Assume you already have:

- a component such as `Counter`
- an effect type such as `StdoutEffect`
- a type that observes that effect

```rust
use eyre::Result;
use vihaco::{Effects, observe};

#[derive(Debug, Clone)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Default)]
pub struct StdoutCollector {
    lines: Vec<String>,
}

#[observe(StdoutEffect)]
impl StdoutCollector {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
```

Now you can compose a runtime root:

```rust
# use eyre::Result;
# use vihaco::{Effects, Instruction, component, observe};
# #[derive(Debug, Clone, Instruction)]
# pub enum CounterInst { Print }
# #[derive(Debug, Default)]
# pub struct Counter;
# #[component(instruction = CounterInst, message = ())]
# impl Counter {
#     fn execute(&mut self, _inst: CounterInst, _msg: ()) -> Result<Effects<()>> { Ok(Effects::none()) }
# }
# #[derive(Debug, Clone)]
# pub struct StdoutEffect(pub String);
# #[derive(Debug, Default)]
# pub struct StdoutCollector { lines: Vec<String> }
# #[observe(StdoutEffect)]
# impl StdoutCollector {
#     fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
#         self.lines.push(effect.0.clone());
#         Ok(Effects::none())
#     }
# }
use vihaco::composite;

#[composite]
#[derive(Debug, Default)]
pub struct CounterComposite {
    #[device(0x00, alias = "count")]
    counter: Counter,

    // A plain observer field — the runtime delivers StdoutEffect to it.
    stdout: StdoutCollector,
}
```

## What `#[composite]` Generates

`#[composite]` is transitional scaffolding that generates the repetitive composition glue from the `#[device(...)]` fields:

- **An outer instruction enum** named `<StructName>Instruction`, with one variant per device field. Each variant is the PascalCase of the field name and wraps that component's instruction type. For `CounterComposite` above the macro emits, roughly:

  ```rust ignore
  #[derive(Debug, Clone, Instruction)]
  pub enum CounterCompositeInstruction {
      Counter(<Counter as GeneratedComponent>::Instruction),
  }
  ```

- **Composite metadata** — an `impl GeneratedMachine` whose `metadata()` returns a `CompositeMetadata` listing each device's code and field name, plus the source-symbol aliases (so a loader can map a name like `"counter"` to its device code).

- **An optional program counter** — if one field is marked `#[program]`, the macro also implements `ProgramCounter` for the composite, delegating `pc` / `pc_mut` / `get_instruction` to that field.

The `#[device]` and `#[program]` attributes are stripped from the struct the macro emits, so they don't leak into your type.

The long-term model is still explicit Rust composition. The macro is convenience for the device dispatch and metadata, not the semantic center of the design — message resolution and effect delivery stay in hand-written runtime code.

## The Field Attributes

### `#[device(CODE, alias = "…")]`

Associates a component field with a device code and optional source aliases.

```rust ignore
#[device(0x00, alias = "count")]
counter: Counter,
```

- `CODE` is a `u8` device code; it must be unique across the composite (a duplicate is a compile error).
- `alias = "…"` registers a source-symbol alias for the field; you can repeat it for multiple aliases. The field name itself is always registered as a source symbol, and every name (field or alias) must be unique across the composite.

The field type must implement `GeneratedComponent` (which `#[component(...)]` provides). The device code and aliases are what a loader uses to validate source symbols and route instructions when a composite loads a module.

### `#[program]`

Marks the field that owns the program counter. When present, the composite gets a `ProgramCounter` impl delegating to that field:

```rust ignore
#[composite]
#[derive(Default)]
pub struct Machine {
    #[program]
    #[device(0x00, alias = "cpu")]
    cpu: Cpu,

    #[device(0x01, alias = "signal")]
    signal: SignalGenerator,
}
```

Here `Machine` drives its instruction pointer through the `cpu` field.

## Effect Continuation Is Hand-Written

`#[composite]` generates the instruction enum and metadata, but it does **not** auto-deliver effects to observers. Continuing effects is something the runtime does explicitly: execute a component, then hand each returned effect to the types that observe it by calling their `Observe` impls.

```rust ignore
use vihaco::{GeneratedComponent, Observe};

impl CounterComposite {
    fn print(&mut self, msg: PrintPrefix) -> eyre::Result<()> {
        // Counter executes Print and returns a StdoutEffect.
        let effects = self.counter.execute_generated(CounterInst::Print, msg)?;
        // Deliver each effect to the observer that handles it.
        for effect in effects {
            Observe::<StdoutEffect>::observe(&mut self.stdout, &effect)?;
        }
        Ok(())
    }
}
```

Conventions to follow when you write that delivery:

- components return `Effects<T>`
- the runtime continues those effects to all matching observer fields
- both standalone observers and components that also observe receive effects through the same `Observe::observe` call
- follow-up effects continue depth-first
- `Effects::Many(...)` is continued left-to-right
- if an observer needs more data, stage it into a richer effect instead of relying on delivery context

## Hand-Written Runtimes

Not every runtime uses a generic step loop. Hand-written runtimes often call `execute_generated(...)` directly, extract the returned effects, and then interpret or re-deliver them themselves.

The common pattern is:

- use `effect = StepOutcome` when a component's direct output is control flow
- define a runtime-local sum-effect enum when a step needs to mix control flow with other follow-up values
- continue that runtime-local effect set in one place, forwarding observer-facing effects as needed

## Design Guidance

- Keep the composite struct explicit and readable.
- Put `#[observe]` on the type that actually consumes the effect.
- Use `#[device(...)]` aliases that match your source model.
- Mark the instruction-pointer-owning field with `#[program]` when the composite drives an instruction pointer.
- Prefer staged effect types over hidden cross-component observer context.
- Let generated code own the device dispatch and metadata; keep effect delivery and message resolution in one clear place in your runtime.

## What Comes Next

At this point you have the core authoring model:

- components execute instructions
- `#[observe]` reacts to delivered effects
- composites generate the device wiring; the runtime resolves messages and continues effects

From here, the next useful step is to apply the same structure to your own domain types and source model.
