# Exponentiation by squaring

The native implementation should retain traversal of the exponent's bits and
the dependent modular products. The compiler may simplify constant remainders
and remove a final square whose result is unused.

There are no internal black-box barriers. The `iterations` input is the
exponent: work grows with its bit length, and set bits determine when the
accumulator is multiplied. The ratio between exponents is not the expected
timing ratio.

See [workload.toml](workload.toml) for the algorithm, numeric bounds, and cases.
