// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser_derive::Parse;

#[derive(Parse)]
#[syntax_class(type)]
struct Type(i64);

fn main() {}
