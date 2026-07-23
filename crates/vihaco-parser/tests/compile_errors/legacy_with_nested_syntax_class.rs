// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[head]
enum Value {
    #[syntax_class(value)]
    Number(i64),
}

fn main() {}
