// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::composite;
#[composite]
struct Parent {
    #[device(100)]
    #[loadable]
    image: u8,
}
fn main() {}
