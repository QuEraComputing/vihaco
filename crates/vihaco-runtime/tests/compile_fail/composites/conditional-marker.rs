// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use vihaco_runtime::field;
#[vihaco_runtime::composite]
#[cfg(any())]
struct Parent {}
fn main() { let _: Option<field!(Parent::child)> = None; }
