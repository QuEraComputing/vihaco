// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(value)]
#[pattern = "$0 $1"]
struct Pair {
    left: i64,
    right: bool,
}

fn main() {}
