// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{composite, field};
#[composite] struct Child {}
#[composite] struct Parent { #[device] child: Child }
fn main() { let _: Option<field!(Parent::child::missing)> = None; }
