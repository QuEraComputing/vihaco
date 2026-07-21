// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_parser::Parse;

#[derive(Parse)]
#[head]
enum Value {
    #[pattern = "$0"]
    Number(i64),
}

fn main() {}
