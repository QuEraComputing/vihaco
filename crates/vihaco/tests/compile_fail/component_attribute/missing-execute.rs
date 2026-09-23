// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{attributes::component, dialect};

dialect! { arith { Add, } }

#[component]
#[instructions { arith::Add }]
struct Missing;

fn main() {}
