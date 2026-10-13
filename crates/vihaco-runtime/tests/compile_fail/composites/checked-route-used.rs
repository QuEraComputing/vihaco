// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::{composite, field};
#[composite] struct Leaf {}
#[composite] struct Expected { #[device] leaf: Leaf }
#[composite] struct Actual { #[device] other: Leaf }
type AliasChild = Actual;
pub use __vihaco_fields_Expected as __vihaco_fields_AliasChild;
#[composite] struct Machine { #[device] child: AliasChild }
type Route = field!(Machine::child::leaf);
fn main() { let _: Option<Route> = None; }
