// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Compute seed^iterations modulo 65521 by exponentiation by squaring.
pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut exponent = iterations;
    let mut base = seed % 65_521;
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result * base % 65_521;
        }
        base = base * base % 65_521;
        exponent >>= 1;
    }
    result
}
