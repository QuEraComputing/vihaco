// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser_derive::Parse;

#[derive(Parse)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    #[pattern = "'load $0"]
    Load(i64),
    #[pattern = "'load $0"]
    Read(i64),
}

fn main() {}
