// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use framework::__private::{Capabilities, Description, Leaf};
use vihaco_runtime as framework;
use framework::composite;
struct Bad;
#[allow(non_snake_case)]
pub mod __vihaco_fields_Bad { pub mod __markers {} pub mod __children {} }
impl Capabilities for Bad {
    const DESCRIPTION: &'static Description = &Description::Component(&[
        &[Leaf {
            dialect: "a",
            mnemonic: "x",
        }],
        &[Leaf {
            dialect: "a",
            mnemonic: "x",
        }],
    ]);
}
#[composite]
#[vihaco(crate = framework)]
struct Parent<T> {
    #[device(100)]
    child: Bad,
    state: T,
}
fn main() {}
