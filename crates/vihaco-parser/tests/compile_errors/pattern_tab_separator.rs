// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    #[pattern = "'load\t$0"]
    Load(i64),
}

fn main() {}
