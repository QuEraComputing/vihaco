// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{attributes::component, dialect};

dialect! { arith { Add, Sub, } }

#[component]
#[instructions { arith::Add, arith::{Add} }]
struct Duplicate;

fn main() {}
