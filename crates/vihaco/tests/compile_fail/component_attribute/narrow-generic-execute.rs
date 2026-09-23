// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Execute, attributes::component, dialect};

dialect! { arith { Add, } }

trait Backend {}
impl Backend for u8 {}

#[component]
#[instructions { arith::Add }]
struct Generic<T>(T);

impl<T: Backend> Execute<arith::Add> for Generic<T> {
    type Message = ();
    type Effect = ();
    type Error = ();
    fn execute(&mut self, _: &arith::Add, _: ()) -> Result<(), ()> { Ok(()) }
}

fn main() {}
