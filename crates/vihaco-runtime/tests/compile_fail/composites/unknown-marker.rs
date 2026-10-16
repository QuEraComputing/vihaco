// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{composite, field};
#[composite]
struct Parent { state: u8 }
fn main() { let _: Option<field!(Parent::missing)> = None; }
