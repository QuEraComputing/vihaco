// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime as framework;
#[composite]
#[vihaco(crate = framework)]
struct Parent {
    #[device]
    #[loadable]
    image: u8,
}
fn main() {
    let _ = Parent::__vihaco_field_image;
}
