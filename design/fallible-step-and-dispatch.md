# Fallible Step and Dispatch

Status: implemented on 2026-10-14. This document records the scoped change to
the current typed runtime.

## Decision

Use `eyre::Result<()>` directly for both `Step::step` and `Dispatch::dispatch`.
Neither trait gains an associated error type.

```rust,ignore
pub trait Step<F, I>
where
    Self: Composite + Field<F>,
    <Self as Field<F>>::Child: Execute<I>,
{
    fn step(&mut self, instruction: I) -> eyre::Result<()>;
}

pub trait Dispatch: Composite {
    type Instruction;

    fn dispatch(&mut self, instruction: Self::Instruction) -> eyre::Result<()>;
}
```

`Execute<I>` retains its existing associated error type. Step implementations
use `?` to convert compatible execution errors into `eyre::Report` and propagate
failures from message preparation or effect handling. Compatible standard errors
implement `std::error::Error + Send + Sync + 'static`; existing reports propagate
unchanged. Infallible execution is also supported.

Generated dispatch returns the selected step's result directly:

```rust,ignore
fn dispatch(&mut self, instruction: Self::Instruction) -> eyre::Result<()> {
    match instruction {
        MachineInstruction::CpuAdd(instruction) => {
            <Self as Step<field!(Machine::cpu), arithmetic::Add>>::step(
                self,
                instruction,
            )
        }
        // Other generated route/instruction arms follow the same pattern.
    }
}
```

The instruction and variant names above are illustrative.

## Caller behavior

Callers either propagate dispatch failures with `?` or handle the returned
report. Drivers may report failures or downcast to concrete error types when
specific recovery is needed. Error handling does not require changing ownership
of the composite or driver.

Returning an error does not imply that state changes or effects have been rolled
back. Recovery and termination remain caller responsibilities.

This approach follows the repository's existing use of eyre and avoids custom
aggregation and handling machinery. It does not provide exhaustive typed error
matching or automatically preserve the destination as structured error metadata.

## Implementation work

1. Update both trait signatures and document when they return errors.
2. Update composite dispatch generation to return each selected step's result.
3. Update step implementations to propagate execution and effect-handling
   failures with `?`, handle successful effects as before, and return `Ok(())`.
   Review CPU fixtures that currently store an execution result on the machine:
   failures should propagate through dispatch while successful outcomes remain
   available to their callers.
4. Update dispatch callers to handle or propagate the returned result.
5. Update trait examples, documentation, cross-crate fixtures, and compile-fail
   fixtures. Refresh affected trybuild diagnostics when necessary.
6. Add coverage for successful execution and error propagation through direct
   and nested destinations, including concrete-error and `Infallible` execution
   implementations.

## Validation

Run focused typed-runtime, derive, and CPU tests with `typed-declarations`
enabled, including cross-crate and trybuild coverage. Run the repository checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
hawkeye check
```

Default workspace checks alone do not exercise feature-gated typed declarations;
run the relevant checks with that feature enabled as well.

## Scope

This is a breaking change to the current typed trait signatures. It does not
introduce generated error enums, `DispatchError<F, I, E>` wrappers,
`HandleError` traits, `handle_with` methods, or error-policy builders. It also
does not implement the future owned-execution and driver lifecycle interfaces
discussed in `rework-2/resolve-step-dispatch.md`.

## Implementation notes

The traits, generated dispatch, step implementations, callers, examples, and
cross-crate/compile-fail fixtures now use the fallible signatures. CPU fixtures
assert `StepOutcome` values directly through `Execute`; their dispatch steps
propagate execution failures without storing outcomes on the composite. Tests cover direct and nested destinations, input preparation,
execution, effect handling, concrete error downcasts, existing report context,
`Infallible` execution, and caller-controlled recovery after CPU failures.

Default workspace checks pass. Feature-enabled tests, Clippy, and doctests pass
for `vihaco-runtime`, `vihaco-runtime-derive`, and `vihaco-cpu`. Whole-workspace
feature-enabled Clippy and doctests encounter legacy `component!` examples in
`vihaco-doctests`, because `typed-declarations` replaces that macro with an
attribute; migrating those legacy guide examples is outside this change.
