// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load the two digitizer programs in section order.

use super::digitizer::{Instruction, Machine};
use eyre::{Result, eyre};
use vihaco_stdlib::Image;

pub(crate) const SOURCE: &str = include_str!("digitizers.sst");
pub(crate) const SECTIONS: [&str; 2] = ["digitizer-a", "digitizer-b"];

pub(crate) fn load(source: &str) -> Result<[Image<Instruction>; 2]> {
    crate::sst::load_sections::<Machine>("digitizers.sst", source, &SECTIONS)?
        .try_into()
        .map_err(|_| eyre!("expected two digitizer sections"))
}
