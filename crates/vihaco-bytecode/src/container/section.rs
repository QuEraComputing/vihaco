// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, ops::Range};

/// One entry of a file's flat section table.
///
/// A section is a named, independently loadable unit: a header plus a code
/// block. The name exists only to select a section; execution relies on
/// addresses inside the code, and any address-to-name mapping belongs in the
/// header metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionEntry {
    pub name: String,
    pub header: Range<usize>,
    pub code: Range<usize>,
}

/// The container-level result of parsing a file: the global context range and
/// the flat section table, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub context: Range<usize>,
    pub sections: Vec<SectionEntry>,
}

/// Validate a section name and record it, rejecting duplicates.
pub(crate) fn claim_section_name(seen: &mut BTreeSet<String>, name: &str) -> eyre::Result<()> {
    if name.is_empty() {
        return Err(eyre::eyre!("section name cannot be empty"));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(eyre::eyre!(
            "section name `{name}` cannot contain whitespace"
        ));
    }
    if !seen.insert(name.to_owned()) {
        return Err(eyre::eyre!("duplicate section `{name}`"));
    }
    Ok(())
}
