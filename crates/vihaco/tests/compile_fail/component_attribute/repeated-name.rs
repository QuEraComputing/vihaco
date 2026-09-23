// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::attributes::component;

#[component]
#[instructions { arith::{Add, Add} }]
struct Duplicate;

fn main() {}
