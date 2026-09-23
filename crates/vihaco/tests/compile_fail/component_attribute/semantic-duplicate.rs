// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, attributes::component, dialect};

dialect! { arith { Add, } }
use arith as alias;

#[component]
#[instructions { arith::Add, alias::Add }]
struct Duplicate;

impl Execute<arith::Add> for Duplicate {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Add, _: ()) -> Result<(), ()> { Ok(()) }
}

fn main() {}
