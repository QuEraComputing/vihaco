// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut x = seed;
    for i in 0..iterations {
        // Carry state across iterations instead of an associative XOR reduction.
        x = ((x << 1) ^ (x >> 15) ^ i) & 65_535;
    }
    x
}
