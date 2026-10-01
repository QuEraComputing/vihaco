// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime as framework;
#[composite]
#[vihaco(crate = framework)]
struct Parent<T> {
    #[device]
    child: T,
}
fn main() {}
