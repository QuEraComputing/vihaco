// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Lower a named Warehouse SST section through its generated instruction set.

use super::{Instruction, Warehouse};
use eyre::{Result, eyre};
use vihaco_stdlib::Image;

pub(super) fn load() -> Result<Image<Instruction>> {
    load_section("warehouse.sst", include_str!("warehouse.sst"), "warehouse")
}

#[cfg(test)]
pub(super) fn load_failure(section: &str) -> Result<Image<Instruction>> {
    load_section("failures.sst", include_str!("failures.sst"), section)
}

fn load_section(filename: &str, source: &str, section: &str) -> Result<Image<Instruction>> {
    crate::sst::load_sections::<Warehouse>(filename, source, &[section])?
        .pop()
        .ok_or_else(|| eyre!("missing Warehouse section {section}"))
}
