// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    ops::Range,
};

use crate::binary::common::validate_local_section_name;

use super::{
    context::BytecodeContext,
    format::{
        ChildSectionTableEntry, ChildSectionTableHeader, SectionBytecodeHeader, SectionFrame,
    },
    section::{SectionNode, SectionPath},
};

impl SectionFrame {
    fn read_at<C>(
        bytes: &[u8],
        section_start: usize,
        path: &SectionPath,
        context: &C,
    ) -> eyre::Result<Self>
    where
        C: BytecodeContext,
    {
        let frame_end = checked_add(section_start, Self::ENCODED_LEN, "section frame end")?;
        let frame_bytes = bytes.get(section_start..frame_end).ok_or_else(|| {
            eyre::eyre!(
                "section `{}` does not contain a complete section frame",
                path.display(context)
            )
        })?;
        let mut cursor = Cursor::new(frame_bytes);
        Self::read_from(&mut cursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SectionParseInfo {
    pub(super) start: usize,
    pub(super) path: SectionPath,
}

/// Parse a section of a bytecode file.
///
/// The vihaco bytecode binary is organized so the structure of
/// the section reads easily from this function.
///
/// All logic related to the parsing of the current section should
/// come _before_ the parsing logic of its children.
pub(super) fn parse_section<C>(
    bytes: &[u8],
    context: &C,
    info: SectionParseInfo,
) -> eyre::Result<SectionNode>
where
    C: BytecodeContext,
{
    let SectionParseInfo {
        start: section_start,
        path,
    } = info;

    let frame = SectionFrame::read_at(bytes, section_start, &path, context)?;
    let section_end = checked_add(section_start, frame.section_len, "section end")?;
    if section_end > bytes.len() {
        return Err(eyre::eyre!(
            "section `{}` extends past end of bytecode",
            path.display(context)
        ));
    }
    if frame.composite_header_len > frame.section_len {
        return Err(eyre::eyre!(
            "section `{}` composite header length {} exceeds section length {}",
            path.display(context),
            frame.composite_header_len,
            frame.section_len
        ));
    }

    let composite_header_start = checked_add(
        section_start,
        SectionFrame::ENCODED_LEN,
        "composite header start",
    )?;
    let composite_header_end = checked_add(
        composite_header_start,
        frame.composite_header_len,
        "composite header end",
    )?;

    // we generate a call to SectionView::parse_header<T: CompositeHeader>
    // if the composite contains a `#[header]`
    let composite_header = composite_header_start..composite_header_end;

    let bytecode_header_start = composite_header.end;
    if checked_add(
        bytecode_header_start,
        SectionBytecodeHeader::ENCODED_LEN,
        "section bytecode header end",
    )? > section_end
    {
        return Err(eyre::eyre!(
            "section `{}` does not contain a complete bytecode header",
            path.display(context)
        ));
    }

    let mut bytecode_header_cursor = Cursor::new(&bytes[bytecode_header_start..section_end]);
    let bytecode_header = SectionBytecodeHeader::read_from(&mut bytecode_header_cursor)?;
    let bytecode_start = checked_add(
        bytecode_header_start,
        SectionBytecodeHeader::ENCODED_LEN,
        "bytecode start",
    )?;
    let bytecode_end = checked_add(bytecode_start, bytecode_header.bytecode_len, "bytecode end")?;
    if bytecode_end > section_end {
        return Err(eyre::eyre!(
            "section `{}` bytecode extends past section end",
            path.display(context)
        ));
    }

    let child_table_start = bytecode_end;
    if checked_add(
        child_table_start,
        ChildSectionTableHeader::ENCODED_LEN,
        "child section table header end",
    )? > section_end
    {
        return Err(eyre::eyre!(
            "section `{}` does not contain a complete child section table header",
            path.display(context)
        ));
    }

    let mut child_table_cursor = Cursor::new(&bytes[child_table_start..section_end]);
    let child_table_header = ChildSectionTableHeader::read_from(&mut child_table_cursor)?;
    let total_len_of_child_section_entries = child_table_header
        .child_count
        .checked_mul(ChildSectionTableEntry::ENCODED_LEN)
        .ok_or_else(|| {
            eyre::eyre!(
                "section `{}` child table length overflows usize",
                path.display(context)
            )
        })?;
    let expected_child_table_len = checked_add(
        ChildSectionTableHeader::ENCODED_LEN,
        total_len_of_child_section_entries,
        "child section table length",
    )?;
    if checked_add(
        child_table_start,
        expected_child_table_len,
        "child section table end",
    )? > section_end
    {
        return Err(eyre::eyre!(
            "section `{}` does not contain a complete child section table",
            path.display(context)
        ));
    }

    let child_section_entries = read_child_section_table(
        &mut child_table_cursor,
        child_table_header.child_count,
        &path,
        context,
    )?;

    let child_table_len = child_table_cursor.position() as usize;
    let child_table_end = checked_add(
        child_table_start,
        child_table_len,
        "child section table end",
    )?;

    let mut claimed_ranges = ClaimedSectionRanges::new(bytecode_start..child_table_end);
    let mut children = Vec::with_capacity(child_section_entries.len());
    for child in child_section_entries {
        children.push(parse_child_section(
            bytes,
            context,
            ChildSectionParseInfo {
                parent_path: &path,
                parent_start: section_start,
                parent_end: section_end,
                parent_child_table_end: child_table_end,
                claimed_ranges: &mut claimed_ranges,
                child,
            },
        )?);
    }

    Ok(SectionNode {
        path,
        section: section_start..section_end,
        header: composite_header,
        bytecode: bytecode_start..bytecode_end,
        children,
    })
}

/// Represents a [`ChildSectionTableEntry`] with its name;
/// we use this for error reporting.
struct ResolvedChildSectionTableEntry {
    name: String,
    entry: ChildSectionTableEntry,
}

fn read_child_section_table<R, C>(
    reader: &mut R,
    child_count: usize,
    parent_path: &SectionPath,
    context: &C,
) -> eyre::Result<Vec<ResolvedChildSectionTableEntry>>
where
    R: Read,
    C: BytecodeContext,
{
    let mut children = Vec::with_capacity(child_count);
    let mut names = BTreeSet::new();
    for _ in 0..child_count {
        let entry = ChildSectionTableEntry::read_from(reader)?;
        let child_name = context
            .section_name(entry.local_name_string)
            .ok_or_else(|| {
                eyre::eyre!(
                    "section `{}` references missing section name string {}",
                    parent_path.display(context),
                    entry.local_name_string
                )
            })?
            .to_string();
        validate_local_section_name(parent_path, context, &child_name)?;
        if !names.insert(child_name.clone()) {
            return Err(eyre::eyre!(
                "section `{}` declares duplicate child `{}`",
                parent_path.display(context),
                child_name
            ));
        }

        children.push(ResolvedChildSectionTableEntry {
            name: child_name,
            entry,
        });
    }
    Ok(children)
}

/// The claimed ranges within a section.
///
/// This helper struct encapsulates the constraint that no ranges in a section's
/// data should overlap. For example, child data cannot overlap with other child data
/// or its parent's bytecode.
struct ClaimedSectionRanges {
    ranges: Vec<(String, Range<usize>)>,
}

impl ClaimedSectionRanges {
    fn new(parent_data: Range<usize>) -> Self {
        Self {
            ranges: vec![("parent data".to_string(), parent_data)],
        }
    }

    fn claim_child<C>(
        &mut self,
        parent_path: &SectionPath,
        context: &C,
        child_name: String,
        range: Range<usize>,
    ) -> eyre::Result<()>
    where
        C: BytecodeContext,
    {
        for (existing_name, existing) in &self.ranges {
            if ranges_overlap(existing, &range) {
                return Err(eyre::eyre!(
                    "section `{}` child `{}` overlaps `{}`",
                    parent_path.display(context),
                    child_name,
                    existing_name
                ));
            }
        }
        self.ranges.push((child_name, range));
        Ok(())
    }
}

struct ChildSectionParseInfo<'a> {
    parent_path: &'a SectionPath,
    parent_start: usize,
    parent_end: usize,
    parent_child_table_end: usize,
    claimed_ranges: &'a mut ClaimedSectionRanges,
    child: ResolvedChildSectionTableEntry,
}

/// This performs all the logic related to validating a child's bounds against
/// its parent's bounds, then recursively parses the child.
fn parse_child_section<C>(
    bytes: &[u8],
    context: &C,
    info: ChildSectionParseInfo<'_>,
) -> eyre::Result<SectionNode>
where
    C: BytecodeContext,
{
    let ChildSectionParseInfo {
        parent_path,
        parent_start,
        parent_end,
        parent_child_table_end,
        claimed_ranges,
        child,
    } = info;
    let ResolvedChildSectionTableEntry {
        name: child_name,
        entry,
    } = child;

    let child_start = checked_add(parent_start, entry.section_offset, "child section start")?;
    let child_path = parent_path.child(entry.local_name_string);
    if child_start < parent_child_table_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` begins before parent child table ends",
            parent_path.display(context),
            child_name
        ));
    }
    if child_start >= parent_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` extends past section end",
            parent_path.display(context),
            child_name
        ));
    }
    if checked_add(
        child_start,
        SectionFrame::ENCODED_LEN,
        "child section frame end",
    )? > parent_end
    {
        return Err(eyre::eyre!(
            "section `{}` child `{}` extends past section end",
            parent_path.display(context),
            child_name
        ));
    }

    let child_frame = SectionFrame::read_at(bytes, child_start, &child_path, context)?;
    let child_end = checked_add(child_start, child_frame.section_len, "child section end")?;
    if child_end > parent_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` extends past section end",
            parent_path.display(context),
            child_name
        ));
    }

    let range = child_start..child_end;
    claimed_ranges.claim_child(parent_path, context, child_name, range)?;

    parse_section(
        bytes,
        context,
        SectionParseInfo {
            start: child_start,
            path: child_path,
        },
    )
}

pub(super) fn checked_add(lhs: usize, rhs: usize, label: &str) -> eyre::Result<usize> {
    lhs.checked_add(rhs)
        .ok_or_else(|| eyre::eyre!("{label} overflows usize"))
}

fn ranges_overlap(lhs: &Range<usize>, rhs: &Range<usize>) -> bool {
    lhs.start < rhs.end && rhs.start < lhs.end
}
