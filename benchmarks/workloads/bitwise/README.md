# Bitwise

The native implementation should retain repeated, dependent rotation, shift,
AND, and XOR updates. Check that the evolving values remain in the loop.
The compiler may combine shifts with other instructions and keep values in
registers.

There are no internal black-box barriers. Work grows with the iteration count.
Python explicitly masks values to match the 64-bit operations in Rust and SST.

See [workload.toml](workload.toml) for the algorithm, numeric bounds, and cases.
