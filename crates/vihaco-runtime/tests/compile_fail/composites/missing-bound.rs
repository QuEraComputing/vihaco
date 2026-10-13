// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime as framework;
use framework::composite;
#[composite]
#[vihaco(crate = framework)]
struct Parent<T> {
    #[device(100)]
    child: T,
}
fn main() {}
