# Arithmetic

The native implementation should retain repeated accumulator updates with
modular arithmetic. Check that the updated accumulator feeds the next iteration
and that the loop remains in the optimized assembly.

The compiler may simplify constant multiplication and remainder operations.
There are no internal black-box barriers. Work grows with the iteration count,
though fixed invocation costs can affect timing ratios for short inputs.

See [workload.toml](workload.toml) for the algorithm, numeric bounds, and cases.
