# Large frame

Each round updates every frame slot using its predecessor, and every final
slot contributes to the returned sum. Check that the optimized native loop
still updates all slots rather than replacing the rounds with a formula.

Rust may keep the slots in registers and unroll the traversal of the frame.
This workload exercises VM locals, but does not force the native implementation
to access memory for each slot. The language comparison therefore includes
those differences in how values are stored.

There are no internal black-box barriers. Work grows with the number of rounds
supplied through `iterations`.

See [workload.toml](workload.toml) for the update order, numeric bounds, and cases.
