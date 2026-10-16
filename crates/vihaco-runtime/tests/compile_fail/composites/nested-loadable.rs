// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{composite, field};
#[composite] struct Child { #[device(100)] #[loadable] image: u8 }
#[composite] struct Parent { #[device(101)] child: Child }
fn main() { let _: Option<field!(Parent::child::image::leaf)> = None; }
