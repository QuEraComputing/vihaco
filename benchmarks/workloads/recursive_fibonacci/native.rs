// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Compute Fibonacci recursively, then add the seed to the result.
pub fn run(iterations: u64, seed: u64) -> u64 {
    fibonacci(iterations) + seed
}

// This workload measures both recursive calls. Keep each returned value opaque
// so the compiler cannot turn one branch into an accumulator loop. The barriers
// are part of the native timing; arithmetic and frame handling remain optimized.
#[inline(never)]
fn fibonacci(n: u64) -> u64 {
    if n < 2 {
        n
    } else {
        std::hint::black_box(fibonacci(n - 1)) + std::hint::black_box(fibonacci(n - 2))
    }
}
