// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser_derive::Parse;

#[derive(Parse)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    #[pattern = "'load `comma"]
    Load,
}

fn main() {}
