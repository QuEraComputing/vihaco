// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser_derive::Parse;

#[derive(Parse)]
#[syntax_class(instruction)]
enum Instruction {
    #[pattern = "'pair $left $right"]
    Pair(i64, bool),
}

fn main() {}
