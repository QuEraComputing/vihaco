// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    #[pattern = "'load `;`"]
    Load,
}

fn main() {}
