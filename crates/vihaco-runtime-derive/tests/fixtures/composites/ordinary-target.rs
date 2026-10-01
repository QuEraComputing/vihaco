// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime as framework;
#[composite]
#[vihaco(crate = framework)]
struct Parent {
    state: u8,
}
fn main() {
    let _ = Parent::__vihaco_field_state;
}
