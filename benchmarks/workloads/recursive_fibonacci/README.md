# Recursive Fibonacci

The native implementation should retain both recursive calls and use both
returned values. Inspect the helper as well as the entry point to check that
one recursive branch has not become an accumulator loop.

The helper disables inlining and black-boxes each recursive result to preserve
those calls. The barriers contribute to native timing. This reference measures
constrained recursive execution rather than unrestricted optimized Fibonacci;
ordinary arithmetic and frame optimizations are still allowed.

The `iterations` input is the Fibonacci index. Execution cost grows with the
recursive call tree, so the ratio between input indices is not the expected
timing ratio.

See [workload.toml](workload.toml) for the recurrence, numeric bounds, and cases.
