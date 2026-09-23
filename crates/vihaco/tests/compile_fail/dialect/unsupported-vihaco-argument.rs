// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::dialect;

dialect! {
    #[vihaco(unknown = crate)]
    invalid { Run }
}

fn main() {}
