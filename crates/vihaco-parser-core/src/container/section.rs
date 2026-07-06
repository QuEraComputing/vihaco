// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::ops::Range;

/// The fully resolved path for a parsed section.
///
/// The root section has no components.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionPath {
    components: Vec<String>,
}

impl SectionPath {
    pub fn root() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    pub fn local_name(&self) -> Option<&str> {
        self.components.last().map(String::as_str)
    }

    pub(crate) fn child(&self, local_name: impl Into<String>) -> Self {
        let mut components = self.components.clone();
        components.push(local_name.into());
        Self { components }
    }
}

impl Default for SectionPath {
    fn default() -> Self {
        Self::root()
    }
}

impl std::fmt::Display for SectionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            return f.write_str("<root>");
        }

        for (index, component) in self.components.iter().enumerate() {
            if index != 0 {
                f.write_str("/")?;
            }
            f.write_str(component)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for SectionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// The implementation-level representation of a parsed section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionNode {
    pub path: SectionPath,
    pub section: Range<usize>,
    pub header: Range<usize>,
    pub bytecode: Range<usize>,
    pub children: Vec<SectionNode>,
}

pub(crate) fn validate_local_section_name(parent: &SectionPath, child: &str) -> eyre::Result<()> {
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
