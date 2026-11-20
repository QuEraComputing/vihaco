// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Binary container framing.
//!
//! All integers are little-endian; offsets are absolute byte positions in the
//! file.
//!
//! ```text
//! magic            [u8; 4] = "VHBC"
//! version          u16
//! flags            u16
//! context_len      u64
//! context          [u8; context_len]
//! section_count    u32
//! section_table    [SectionTableEntry; section_count]
//! data             header and code bytes referenced by the table
//!
//! SectionTableEntry:
//!   name_string    u32   (resolved through the global context)
//!   header_offset  u64
//!   header_len     u64
//!   code_offset    u64
//!   code_len       u64
//! ```

use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    ops::Range,
};

use byteorder::{LittleEndian, ReadBytesExt};

use super::{ParsedFile, SectionEntry, section::claim_section_name};

pub const MAGIC: &[u8; 4] = b"VHBC";
pub const VERSION: u16 = 2;
pub const FLAGS: u16 = 0;

/// Encoded size of the fixed file header that precedes the global context.
pub const FILE_HEADER_LEN: usize = 4 + 2 + 2 + 8;
/// Encoded size of the section count that precedes the section table.
pub const SECTION_COUNT_LEN: usize = 4;
/// Encoded size of one section table entry.
pub const SECTION_TABLE_ENTRY_LEN: usize = 4 + 8 * 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionTableEntry {
    name_string: u32,
    header_offset: usize,
    header_len: usize,
    code_offset: usize,
    code_len: usize,
}

impl SectionTableEntry {
    fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            name_string: reader.read_u32::<LittleEndian>()?,
            header_offset: read_usize_u64(reader, "section header offset")?,
            header_len: read_usize_u64(reader, "section header length")?,
            code_offset: read_usize_u64(reader, "section code offset")?,
            code_len: read_usize_u64(reader, "section code length")?,
        })
    }
}

pub fn parse_file<F>(bytes: &[u8], mut section_name: F) -> eyre::Result<ParsedFile>
where
    F: FnMut(u32) -> Option<String>,
{
    let context = context_range(bytes)?;

    let count_end = checked_add(context.end, SECTION_COUNT_LEN, "section count end")?;
    let count_bytes = bytes
        .get(context.end..count_end)
        .ok_or_else(|| eyre::eyre!("bytecode does not contain a section count"))?;
    let section_count = Cursor::new(count_bytes).read_u32::<LittleEndian>()? as usize;

    let table_len = section_count
        .checked_mul(SECTION_TABLE_ENTRY_LEN)
        .ok_or_else(|| eyre::eyre!("section table length overflows usize"))?;
    let table_end = checked_add(count_end, table_len, "section table end")?;
    let table_bytes = bytes
        .get(count_end..table_end)
        .ok_or_else(|| eyre::eyre!("bytecode does not contain a complete section table"))?;

    let mut reader = Cursor::new(table_bytes);
    let mut names = BTreeSet::new();
    let mut claimed: Vec<(String, Range<usize>)> = Vec::new();
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let entry = SectionTableEntry::read_from(&mut reader)?;
        let name = section_name(entry.name_string).ok_or_else(|| {
            eyre::eyre!(
                "section table references missing section name string {}",
                entry.name_string
            )
        })?;
        claim_section_name(&mut names, &name)?;

        let header = data_range(
            bytes,
            table_end,
            &name,
            "header",
            entry.header_offset,
            entry.header_len,
        )?;
        let code = data_range(
            bytes,
            table_end,
            &name,
            "code",
            entry.code_offset,
            entry.code_len,
        )?;
        for (label, range) in [("header", &header), ("code", &code)] {
            let label = format!("section `{name}` {label}");
            for (existing_label, existing) in &claimed {
                if ranges_overlap(existing, range) {
                    return Err(eyre::eyre!("{label} overlaps {existing_label}"));
                }
            }
            claimed.push((label, range.clone()));
        }

        sections.push(SectionEntry { name, header, code });
    }

    Ok(ParsedFile { context, sections })
}

pub fn context_range(bytes: &[u8]) -> eyre::Result<Range<usize>> {
    let mut cursor = Cursor::new(bytes);

    let mut magic = [0; 4];
    cursor
        .read_exact(&mut magic)
        .map_err(|_| eyre::eyre!("bytecode is too short to contain a header"))?;
    if &magic != MAGIC {
        return Err(eyre::eyre!("invalid bytecode magic"));
    }

    let version = cursor.read_u16::<LittleEndian>()?;
    if version != VERSION {
        return Err(eyre::eyre!(
            "unsupported bytecode version {} (expected {})",
            version,
            VERSION
        ));
    }

    let flags = cursor.read_u16::<LittleEndian>()?;
    if flags != FLAGS {
        return Err(eyre::eyre!("unsupported bytecode flags 0x{flags:04X}"));
    }

    let context_len = read_usize_u64(&mut cursor, "global context length")?;
    let context_end = checked_add(FILE_HEADER_LEN, context_len, "global context end")?;
    if bytes.get(FILE_HEADER_LEN..context_end).is_none() {
        return Err(eyre::eyre!("global context is out of bounds"));
    }
    Ok(FILE_HEADER_LEN..context_end)
}

/// Resolve and bounds-check one data range referenced by the section table.
fn data_range(
    bytes: &[u8],
    data_start: usize,
    name: &str,
    label: &str,
    offset: usize,
    len: usize,
) -> eyre::Result<Range<usize>> {
    let end = checked_add(offset, len, "section data end")?;
    if len != 0 && offset < data_start {
        return Err(eyre::eyre!(
            "section `{name}` {label} begins before the section table ends"
        ));
    }
    if end > bytes.len() {
        return Err(eyre::eyre!(
            "section `{name}` {label} extends past end of bytecode"
        ));
    }
    Ok(offset..end)
}

pub fn checked_add(lhs: usize, rhs: usize, label: &str) -> eyre::Result<usize> {
    lhs.checked_add(rhs)
        .ok_or_else(|| eyre::eyre!("{label} overflows usize"))
}

/// Whether two ranges share a byte. An empty range occupies no bytes, so it
/// never overlaps anything, wherever it points.
fn ranges_overlap(lhs: &Range<usize>, rhs: &Range<usize>) -> bool {
    !lhs.is_empty() && !rhs.is_empty() && lhs.start < rhs.end && rhs.start < lhs.end
}

fn read_usize_u64<R: Read>(reader: &mut R, label: &str) -> eyre::Result<usize> {
    usize::try_from(reader.read_u64::<LittleEndian>()?)
        .map_err(|_| eyre::eyre!("{label} does not fit in usize"))
}
