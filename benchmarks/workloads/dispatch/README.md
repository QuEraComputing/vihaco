# Dispatch

The shift/XOR accumulator carries a dependency between iterations. Check that
the native loop retains that dependency rather than replacing the computation
with a closed-form result. Instruction combining and loop unrolling are
acceptable.

There are no internal black-box barriers. Work grows with the iteration count.
The whole-program measurement includes the loop and accumulator updates; use
the suite's isolated instruction measurements to examine individual dispatch
paths.

See [workload.toml](workload.toml) for the algorithm, numeric bounds, and cases.
