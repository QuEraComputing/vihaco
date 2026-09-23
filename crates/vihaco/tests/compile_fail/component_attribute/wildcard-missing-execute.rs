// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, attributes::component, dialect};

dialect! { arith { Add, New, } }

#[component]
#[instructions { arith::* }]
struct MissingNew;

impl Execute<arith::Add> for MissingNew {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Add, _: ()) -> Result<(), ()> { Ok(()) }
}

fn main() {}
