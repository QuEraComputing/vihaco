// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::dialect;

dialect! {
    duplicate {
        Run,
        #[pattern = "'run $0"]
        RunImmediate(u32),
    }
}

fn main() {}
