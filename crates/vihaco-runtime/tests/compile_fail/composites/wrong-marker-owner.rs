// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{composite, field};
use vihaco_runtime::Field;
#[composite]
struct Child {}
#[composite]
struct Left { #[device(100)] child: Child }
#[composite]
struct Right { #[device(101)] child: Child }
type Wrong = <Left as Field<field!(Right::child)>>::Child;
fn main() { let _: Option<Wrong> = None; }
