// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[cfg_attr(feature = "program-image-execution", path = "values/arithmetic.rs")]
mod arithmetic;

#[cfg_attr(feature = "program-image-execution", path = "values/bitwise.rs")]
mod bitwise;

#[cfg_attr(feature = "program-image-execution", path = "values/comparisons.rs")]
mod comparisons;

#[cfg(not(feature = "program-image-execution"))]
mod constants;

#[cfg(not(feature = "program-image-execution"))]
mod control;

#[cfg_attr(feature = "program-image-execution", path = "values/heap.rs")]
mod heap;

#[cfg(not(feature = "program-image-execution"))]
mod locals;

#[cfg_attr(feature = "program-image-execution", path = "values/logic.rs")]
mod logic;
