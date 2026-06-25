// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{BytecodeContext, SectionPath};

pub(super) fn validate_local_section_name<C>(
    parent: &SectionPath,
    context: &C,
    child: &str,
) -> eyre::Result<()>
where
    C: BytecodeContext,
{
    if child.is_empty() {
        return Err(eyre::eyre!(
            "section `{}` has an empty child name",
            parent.display(context)
        ));
    }
    if child.contains('/') {
        return Err(eyre::eyre!(
            "section `{}` child name `{}` must be a local name",
            parent.display(context),
            child
        ));
    }
    Ok(())
}
