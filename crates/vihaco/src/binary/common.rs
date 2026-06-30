// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::SectionPath;

pub(super) fn validate_local_section_name(parent: &SectionPath, child: &str) -> eyre::Result<()> {
    if child.is_empty() {
        return Err(eyre::eyre!("section `{}` has an empty child name", parent));
    }
    if child.contains('/') {
        return Err(eyre::eyre!(
            "section `{}` child name `{}` must be a local name",
            parent,
            child
        ));
    }
    Ok(())
}
