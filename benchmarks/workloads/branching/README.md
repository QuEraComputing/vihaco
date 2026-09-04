# Branching

The native implementation should retain repeated state-dependent selection
and accumulator updates. The selected value must feed the next iteration.
Conditional-select instructions are acceptable replacements for machine
branches; this workload does not require a particular branch instruction.

There are no internal black-box barriers. Work grows with the iteration count,
but the state determines which updates the algorithm performs.

See [workload.toml](workload.toml) for the algorithm, numeric bounds, and cases.
