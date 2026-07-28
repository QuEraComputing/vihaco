// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(value)]
enum Value {
    #[pattern = "'number $0"]
    Number(i64),
}

fn main() {}
