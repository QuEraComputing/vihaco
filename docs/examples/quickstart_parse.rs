// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco::{Parse, chumsky::Parser as _, dialect};

#[dialect]
pub mod counter {
    #[instruction]
    pub struct Add(pub u64);
}

fn main() {
    let instruction = counter::Add::parser()
        .parse("add 5")
        .into_result()
        .expect("valid add instruction");
    assert_eq!(instruction.0, 5);
}
