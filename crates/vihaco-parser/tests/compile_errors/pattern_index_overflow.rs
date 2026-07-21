// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(instruction)]
enum Instruction {
    #[pattern = "'load $4294967296"]
    Load(i64),
}

fn main() {}
