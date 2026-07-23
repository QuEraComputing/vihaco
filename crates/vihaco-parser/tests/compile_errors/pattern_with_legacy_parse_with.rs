// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[syntax_class(value)]
enum Value {
    Number(#[parse_with = "custom_parser"] i64),
}

fn main() {}
