// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{attributes::component, dialect};

dialect! { arith { Add, Sub, } }

#[component]
#[instructions { arith::*, arith::Add }]
struct Overlap;

fn main() {}
