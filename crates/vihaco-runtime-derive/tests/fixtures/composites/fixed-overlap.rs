// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use framework::__private::{Capabilities, Description, Leaf};
use vihaco_runtime as framework;
struct Bad;
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
    #[device]
    child: Bad,
    state: T,
}
fn main() {}
