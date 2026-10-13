// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime as framework;
use framework::composite;
#[composite]
#[vihaco(crate = framework)]
struct Parent {
    #[device]
    #[loadable]
    image: u8,
}
fn main() {
    let _: Option<framework::field!(Parent::image)> = None;
}
