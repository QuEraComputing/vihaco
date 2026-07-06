// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    ops::Range,
};

use byteorder::{LittleEndian, ReadBytesExt};

use super::{
    section::{validate_local_section_name, SectionNode, SectionPath},
    ParsedFile,
};

pub const MAGIC: &[u8; 4] = b"VHBC";
pub const VERSION: u16 = 1;
pub const FLAGS: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BytecodeHeader {
    context_len: usize,
}

impl BytecodeHeader {
    const CONTEXT_LEN_SIZE: usize = 8;
    const ENCODED_LEN: usize = 4 + 2 + 2 + Self::CONTEXT_LEN_SIZE;

    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(eyre::eyre!("invalid bytecode magic"));
        }

        let version = reader.read_u16::<LittleEndian>()?;
        if version != VERSION {
            return Err(eyre::eyre!(
                "unsupported bytecode version {} (expected {})",
                version,
                VERSION
            ));
        }

        let flags = reader.read_u16::<LittleEndian>()?;
        if flags != FLAGS {
            return Err(eyre::eyre!("unsupported bytecode flags 0x{flags:04X}"));
        }

        Ok(Self {
            context_len: read_usize_u64(reader, "program context length")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionFrame {
    section_len: usize,
    composite_header_len: usize,
}

impl SectionFrame {
    const SECTION_LEN_SIZE: usize = 8;
    const HEADER_LEN_SIZE: usize = 8;
    const ENCODED_LEN: usize = Self::SECTION_LEN_SIZE + Self::HEADER_LEN_SIZE;

    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            section_len: read_usize_u64(reader, "section length")?,
            composite_header_len: read_usize_u64(reader, "composite header length")?,
        })
    }

    fn read_at(bytes: &[u8], section_start: usize, path: &SectionPath) -> eyre::Result<Self> {
        let frame_end = checked_add(section_start, Self::ENCODED_LEN, "section frame end")?;
        let frame_bytes = bytes.get(section_start..frame_end).ok_or_else(|| {
            eyre::eyre!(
                "section `{}` does not contain a complete section frame",
                path
            )
        })?;
        let mut cursor = Cursor::new(frame_bytes);
        Self::read_from(&mut cursor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionBytecodeHeader {
    bytecode_len: usize,
}

impl SectionBytecodeHeader {
    const BYTECODE_LEN_SIZE: usize = 8;
    const ENCODED_LEN: usize = Self::BYTECODE_LEN_SIZE;

    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            bytecode_len: read_usize_u64(reader, "bytecode length")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChildSectionTableHeader {
    child_count: usize,
}

impl ChildSectionTableHeader {
    const CHILD_COUNT_SIZE: usize = 4;
    const ENCODED_LEN: usize = Self::CHILD_COUNT_SIZE;

    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            child_count: reader.read_u32::<LittleEndian>()? as usize,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChildSectionTableEntry {
    local_name_string: u32,
    section_offset: usize,
}

impl ChildSectionTableEntry {
    const LOCAL_NAME_STRING_SIZE: usize = 4;
    const SECTION_OFFSET_SIZE: usize = 8;
    const ENCODED_LEN: usize = Self::LOCAL_NAME_STRING_SIZE + Self::SECTION_OFFSET_SIZE;

    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            local_name_string: reader.read_u32::<LittleEndian>()?,
            section_offset: read_usize_u64(reader, "child section offset")?,
        })
    }
}

pub fn parse_file<F>(bytes: &[u8], mut section_name: F) -> eyre::Result<ParsedFile>
where
    F: FnMut(u32) -> Option<String>,
{
    let context = context_range(bytes)?;

    let root = parse_section(
        bytes,
        &mut section_name,
        SectionParseInfo {
            start: context.end,
            path: SectionPath::root(),
        },
    )?;
    if root.section.end != bytes.len() {
        return Err(eyre::eyre!(
            "bytecode length mismatch: root section describes {} bytes, file has {} bytes",
            root.section.end,
            bytes.len()
        ));
    }

    Ok(ParsedFile { context, root })
}

pub fn context_range(bytes: &[u8]) -> eyre::Result<Range<usize>> {
    let mut cursor = Cursor::new(bytes);
    let header = BytecodeHeader::read_from(&mut cursor)?;
    let context_start = BytecodeHeader::ENCODED_LEN;
    let context_end = checked_add(context_start, header.context_len, "program context end")?;
    if bytes.get(context_start..context_end).is_none() {
        return Err(eyre::eyre!("program context is out of bounds"));
    }
    Ok(context_start..context_end)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SectionParseInfo {
    start: usize,
    path: SectionPath,
}

fn parse_section<F>(
    bytes: &[u8],
    section_name: &mut F,
    info: SectionParseInfo,
) -> eyre::Result<SectionNode>
where
    F: FnMut(u32) -> Option<String>,
{
    let SectionParseInfo {
        start: section_start,
        path,
    } = info;

    let frame = SectionFrame::read_at(bytes, section_start, &path)?;
    let section_end = checked_add(section_start, frame.section_len, "section end")?;
    if section_end > bytes.len() {
        return Err(eyre::eyre!(
            "section `{}` extends past end of bytecode",
            path
        ));
    }
    if frame.composite_header_len > frame.section_len {
        return Err(eyre::eyre!(
            "section `{}` composite header length {} exceeds section length {}",
            path,
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
            path
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
            path
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
            path
        ));
    }

    let mut child_table_cursor = Cursor::new(&bytes[child_table_start..section_end]);
    let child_table_header = ChildSectionTableHeader::read_from(&mut child_table_cursor)?;
    let total_len_of_child_section_entries = child_table_header
        .child_count
        .checked_mul(ChildSectionTableEntry::ENCODED_LEN)
        .ok_or_else(|| eyre::eyre!("section `{}` child table length overflows usize", path))?;
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
            path
        ));
    }

    let child_section_entries = read_child_section_table(
        &mut child_table_cursor,
        child_table_header.child_count,
        &path,
        section_name,
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
            section_name,
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

struct ResolvedChildSectionTableEntry {
    name: String,
    entry: ChildSectionTableEntry,
}

fn read_child_section_table<R, F>(
    reader: &mut R,
    child_count: usize,
    parent_path: &SectionPath,
    section_name: &mut F,
) -> eyre::Result<Vec<ResolvedChildSectionTableEntry>>
where
    R: Read,
    F: FnMut(u32) -> Option<String>,
{
    let mut children = Vec::with_capacity(child_count);
    let mut names = BTreeSet::new();
    for _ in 0..child_count {
        let entry = ChildSectionTableEntry::read_from(reader)?;
        let child_name = section_name(entry.local_name_string).ok_or_else(|| {
            eyre::eyre!(
                "section `{}` references missing section name string {}",
                parent_path,
                entry.local_name_string
            )
        })?;
        validate_local_section_name(parent_path, &child_name)?;
        if !names.insert(child_name.clone()) {
            return Err(eyre::eyre!(
                "section `{}` declares duplicate child `{}`",
                parent_path,
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

struct ClaimedSectionRanges {
    ranges: Vec<(String, Range<usize>)>,
}

impl ClaimedSectionRanges {
    fn new(parent_data: Range<usize>) -> Self {
        Self {
            ranges: vec![("parent data".to_string(), parent_data)],
        }
    }

    fn claim_child(
        &mut self,
        parent_path: &SectionPath,
        child_name: String,
        range: Range<usize>,
    ) -> eyre::Result<()> {
        for (existing_name, existing) in &self.ranges {
            if ranges_overlap(existing, &range) {
                return Err(eyre::eyre!(
                    "section `{}` child `{}` overlaps `{}`",
                    parent_path,
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

fn parse_child_section<F>(
    bytes: &[u8],
    section_name: &mut F,
    info: ChildSectionParseInfo<'_>,
) -> eyre::Result<SectionNode>
where
    F: FnMut(u32) -> Option<String>,
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
    let child_path = parent_path.child(child_name.clone());
    if child_start < parent_child_table_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` begins before parent child table ends",
            parent_path,
            child_name
        ));
    }
    if child_start >= parent_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` extends past section end",
            parent_path,
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
            parent_path,
            child_name
        ));
    }

    let child_frame = SectionFrame::read_at(bytes, child_start, &child_path)?;
    let child_end = checked_add(child_start, child_frame.section_len, "child section end")?;
    if child_end > parent_end {
        return Err(eyre::eyre!(
            "section `{}` child `{}` extends past section end",
            parent_path,
            child_name
        ));
    }

    let range = child_start..child_end;
    claimed_ranges.claim_child(parent_path, child_name, range)?;

    parse_section(
        bytes,
        section_name,
        SectionParseInfo {
            start: child_start,
            path: child_path,
        },
    )
}

pub fn checked_add(lhs: usize, rhs: usize, label: &str) -> eyre::Result<usize> {
    lhs.checked_add(rhs)
        .ok_or_else(|| eyre::eyre!("{label} overflows usize"))
}

fn ranges_overlap(lhs: &Range<usize>, rhs: &Range<usize>) -> bool {
    lhs.start < rhs.end && rhs.start < lhs.end
}

fn read_usize_u64<R: Read>(reader: &mut R, label: &str) -> eyre::Result<usize> {
    usize::try_from(reader.read_u64::<LittleEndian>()?)
        .map_err(|_| eyre::eyre!("{label} does not fit in usize"))
}
