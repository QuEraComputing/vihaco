// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(instruction)]
enum Instruction {
    #[pattern = "'pair $0"]
    Pair(i64, bool),
}

fn main() {}
