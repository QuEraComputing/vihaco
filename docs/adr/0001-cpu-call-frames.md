---
status: accepted
---

# CPU call frames and composite-owned local counts

CPU function frames have one indexed locals region, with parameters first,
followed by a separate operand region. Each composite computes local storage
requirements while resolving its own surface instruction enum, and supplies the
selected function metadata to CPU through a message when executing a call. The
CPU only owns frame mechanics.

## Frame layout and execution

```text
[previous frames][locals: parameters, then additional slots][operands]
                  ^ base                                    ^ operands_index()
```

- The local count includes all parameter slots and therefore is always at least
  the function arity. Load/store use `base + index`;
  there is no separate `locals_index()`.
- The caller explicitly loads or computes arguments onto its operand stack.
  Call reuses its top `arity` operands as the callee's parameter slots, placing
  the new base at `stack.len() - arity`.
- Call resizes the stack to `base + local_count`, filling additional local slots
  with the default word value, zero. The callee starts with no operands.
- The frame records enough information to derive
  `operands_index() = base + local_count`.
- Store no longer grows the stack. Local accesses stay within the established
  locals region, and operand operations cannot consume locals or previous frames.
- Direct calls, indirect calls, and program entry share frame-setup logic.
- Return requires at least the requested number of operands, preserves the top
  requested values, discards the rest of the frame, and restores the caller.
  Terminal return retains the existing returned-values mechanism.

## Resolution and composition

Each composite computes the maximum of function arity and every referenced
load/store index plus one while resolving its own instruction enum. Reads,
writes, and unreachable instructions all contribute; index arithmetic must not
overflow. `FunctionInfo` stores the resulting count, and the composite supplies
that count through `CPUMessage`. A shared resolver or device-to-count mapping is
intentionally deferred.

## Scope and consequences

The CPU validates that the supplied local count is at least the call arity;
composites remain responsible for deriving the count from their resolved
instruction bodies.
Function ownership and cross-device frame-usage validation are also deferred.
Calling on one CPU establishes a frame only on that CPU; recording requirements
for another CPU does not create a frame there.

The benchmark SST programs on `rob/add-benchmarking` already use shared
parameter/local indexing and explicit operand preparation. Their resolver and
entry fixture use the composite-owned count, but the
agreed indexing and argument-passing semantics do not require new SST loads.
Entry-frame allocation is currently outside benchmark timing; repeated calls
with additional locals need separate coverage to measure initialization cost.

Implementation checks should cover sparse and absent local references, multiple
CPU devices, generated conversions, preserved arguments, zero-initialized locals,
operand/local boundaries, nested and indirect calls, entry setup, and returns.
