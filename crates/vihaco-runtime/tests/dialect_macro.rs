// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use chumsky::Parser as _;
use vihaco_runtime::{Parse as _, dialect};

dialect! {
    direct {
        Set(u32 => u64),
        Reset,
    }
}

#[test]
fn dialect_macro_works_through_the_runtime_crate() {
    let syntax = direct::syntax::Set::parser()
        .parse("direct.set 3")
        .into_result()
        .unwrap();
    assert_eq!(syntax, direct::syntax::Set(3_u32));

    assert_eq!(direct::Set(3_u64), direct::Set(3));
    assert_eq!(direct::Reset, direct::Reset);
}
