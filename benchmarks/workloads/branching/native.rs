// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut x = seed;
    for i in 0..iterations {
        if x.is_multiple_of(2) {
            x = x / 2 + i;
        } else {
            x = (3 * x + 1) % 65_521;
        }
    }
    x
}
