// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Mix two 64-bit words with rotations, a truncating shift, AND, and XOR.
pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut value = seed;
    let mut other = 305_419_896_u64;
    for _ in 0..iterations {
        value = (value ^ other).rotate_right(13);
        value = ((value << 7) & other) ^ seed;
        other = other.rotate_left(3);
    }
    value & 65_535
}
