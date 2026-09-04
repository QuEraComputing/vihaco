// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Update every frame slot using the previously updated neighbor.
pub fn run(iterations: u64, seed: u64) -> u64 {
    let mut frame = std::array::from_fn::<_, 10, _>(|index| seed + index as u64);
    for _ in 0..iterations {
        let mut previous = frame[9];
        for slot in &mut frame {
            *slot = (*slot * 17 + previous) % 65_521;
            previous = *slot;
        }
    }
    frame.iter().sum()
}
