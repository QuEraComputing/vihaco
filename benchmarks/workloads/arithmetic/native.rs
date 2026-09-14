// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut x = seed;
    for i in 0..iterations {
        x = (17 * x + i) % 65_521;
    }
    x
}
