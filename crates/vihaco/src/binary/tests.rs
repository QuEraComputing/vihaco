// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use crate::binary::NoContext;
use crate::binary::file::{BytecodeFile, SstFile};
use crate::traits::{FromBytes, FromText, WriteBytes};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Read;

const SECTION_FRAME_LEN: usize = 8 + 8;
const SECTION_BYTECODE_HEADER_LEN: usize = 8;
const CHILD_SECTION_TABLE_HEADER_LEN: usize = 4;
const CHILD_SECTION_TABLE_ENTRY_LEN: usize = 4 + 8;

#[derive(Debug, Clone, PartialEq, crate::Instruction)]
enum TestInst {
    Nop,
    Load(ConstantId),
}

#[derive(Debug, Clone, PartialEq)]
struct WrappedContext {
    inner: SectionNameContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SectionNameContext {
    section_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextContext {
    raw: String,
    section_names: Vec<String>,
}

impl SectionNameResolver for WrappedContext {
    fn section_name(&self, index: u32) -> Option<&str> {
        self.inner.section_name(index)
    }
}

impl SectionNameResolver for SectionNameContext {
    fn section_name(&self, index: u32) -> Option<&str> {
        self.section_names.get(index as usize).map(String::as_str)
    }
}

impl BytecodeGlobalContext for SectionNameContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let mut cursor = bytes;
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut section_names = Vec::with_capacity(count);
        for _ in 0..count {
            section_names.push(read_string(&mut cursor)?);
        }
        if !cursor.is_empty() {
            return Err(eyre::eyre!(
                "section-name context has {} trailing bytes",
                cursor.len()
            ));
        }
        Ok(Self { section_names })
    }
}

impl BytecodeGlobalContext for WrappedContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        Ok(Self {
            inner: SectionNameContext::from_bytes(bytes)?,
        })
    }
}

impl SstGlobalContext for WrappedContext {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(Self {
            inner: <TextContext as SstGlobalContext>::from_text(text)?.into(),
        })
    }
}

impl From<TextContext> for SectionNameContext {
    fn from(context: TextContext) -> Self {
        Self {
            section_names: context.section_names,
        }
    }
}

impl SectionNameResolver for TextContext {
    fn section_name(&self, index: u32) -> Option<&str> {
        self.section_names.get(index as usize).map(String::as_str)
    }
}

impl BytecodeGlobalContext for TextContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let raw = std::str::from_utf8(bytes)?.to_string();
        let section_names = raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(Self { raw, section_names })
    }
}

impl SstGlobalContext for TextContext {
    fn from_text(text: &str) -> eyre::Result<Self> {
        let raw = text.to_string();
        let section_names = raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(Self { raw, section_names })
    }
}

#[test]
fn parses_binary_context_and_nested_sections() {
    const CPU_NAME: u32 = 1;
    const ALU_NAME: u32 = 2;

    let context = context_bytes();
    let alu_header = b"alu header";
    let cpu_header = b"cpu header";
    let root_header = b"root header";
    let alu = binary_section_bytes(alu_header, &[TestInst::Nop], vec![]);
    let cpu = binary_section_bytes(
        cpu_header,
        &[TestInst::Load(ConstantId(0))],
        vec![(ALU_NAME, alu)],
    );
    let root = binary_section_bytes(root_header, &[], vec![(CPU_NAME, cpu)]);
    let file = binary_file_bytes(context, root);

    let parsed: BytecodeFile<SectionNameContext> = BytecodeFile::from_bytes(file).unwrap();

    assert_eq!(
        parsed.context().section_names,
        vec!["main".to_string(), "cpu".to_string(), "alu".to_string()]
    );

    let root = parsed.root();
    assert!(root.path().is_root());
    assert!(root.path().components().is_empty());
    assert_eq!(root.local_name(), None);
    assert_eq!(root.display_path().to_string(), "<root>");
    assert_eq!(root.header_bytes(), b"root header");

    let cpu = root.child("cpu").unwrap();
    assert_eq!(path_components(cpu.path()), vec!["cpu"]);
    assert_eq!(cpu.local_name(), Some("cpu"));
    assert_eq!(cpu.display_path().to_string(), "cpu");
    assert_eq!(cpu.header_bytes(), b"cpu header");
    assert_eq!(
        cpu.decode_instructions::<TestInst>().unwrap(),
        vec![TestInst::Load(ConstantId(0))]
    );

    let alu = cpu.child("alu").unwrap();
    assert_eq!(path_components(alu.path()), vec!["cpu", "alu"]);
    assert_eq!(alu.local_name(), Some("alu"));
    assert_eq!(alu.display_path().to_string(), "cpu/alu");
    assert_eq!(alu.header_bytes(), b"alu header");
    assert_eq!(
        alu.decode_instructions::<TestInst>().unwrap(),
        vec![TestInst::Nop]
    );
}

#[test]
fn parses_binary_file_with_custom_context_representation() {
    const CPU_NAME: u32 = 1;

    let cpu = binary_section_bytes(b"", &[TestInst::Nop], vec![]);
    let root = binary_section_bytes(b"", &[], vec![(CPU_NAME, cpu)]);
    let parsed: BytecodeFile<WrappedContext> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();

    assert_eq!(
        parsed.context().inner.section_names,
        vec!["main".to_string(), "cpu".to_string(), "alu".to_string()]
    );
    assert_eq!(
        parsed.root().child("cpu").unwrap().local_name(),
        Some("cpu")
    );
}

#[test]
fn binary_decode_header_consumes_the_whole_header() {
    #[derive(Debug, PartialEq)]
    struct Header(u32);

    impl FromBytes for Header {
        fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
            Ok(Header(bytes.read_u32::<LittleEndian>()?))
        }
    }

    impl FromText for Header {
        fn from_text(text: &str) -> eyre::Result<Self> {
            Ok(Header(text.parse()?))
        }
    }

    impl WriteBytes for Header {
        fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
            io.write_u32::<LittleEndian>(self.0)?;
            Ok(())
        }
    }

    let mut header = Vec::new();
    header.write_u32::<LittleEndian>(99).unwrap();
    let root = binary_section_bytes(&header, &[], vec![]);
    let parsed: BytecodeFile<SectionNameContext> =
        BytecodeFile::from_bytes(binary_file_bytes(empty_context_bytes(), root)).unwrap();

    assert_eq!(parsed.root().decode_header::<Header>().unwrap(), Header(99));
}

#[test]
fn rejects_bad_binary_magic() {
    let mut bytes = binary_file_bytes(
        empty_context_bytes(),
        binary_section_bytes(b"", &[], vec![]),
    );
    bytes[0] = b'X';

    let err = BytecodeFile::<SectionNameContext>::from_bytes(bytes).unwrap_err();
    assert!(err.to_string().contains("invalid bytecode magic"));
}

#[test]
fn rejects_binary_missing_section_name_string() {
    let root = raw_binary_section(b"", b"", vec![(0, b"")]);
    let err = BytecodeFile::<SectionNameContext>::from_bytes(binary_file_bytes(
        empty_context_bytes(),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("missing section name string"));
}

#[test]
fn rejects_binary_duplicate_child_names() {
    let child_a = binary_section_bytes(b"", &[], vec![]);
    let child_b = binary_section_bytes(b"", &[], vec![]);
    let root = raw_binary_section(b"", b"", vec![(0, &child_a), (0, &child_b)]);

    let err = BytecodeFile::<SectionNameContext>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("duplicate child"));
}

#[test]
fn rejects_binary_out_of_bounds_child_section() {
    let root = raw_binary_section_with_entry_offsets(b"", b"", vec![(0, 999, b"")]);

    let err = BytecodeFile::<SectionNameContext>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("extends past section end"));
}

#[test]
fn rejects_binary_overlapping_child_sections() {
    let child = binary_section_bytes(b"", &[], vec![]);
    let child_offset = (SECTION_FRAME_LEN
        + SECTION_BYTECODE_HEADER_LEN
        + CHILD_SECTION_TABLE_HEADER_LEN
        + (2 * CHILD_SECTION_TABLE_ENTRY_LEN)) as u64;
    let root = raw_binary_section_with_entry_offsets(
        b"",
        b"",
        vec![(0, child_offset, &child), (1, child_offset, &[])],
    );

    let err = BytecodeFile::<SectionNameContext>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu", "gpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("overlaps"));
}

#[test]
fn rejects_binary_bytecode_that_extends_past_section_end() {
    let mut root = Vec::new();
    root.write_u64::<LittleEndian>((SECTION_FRAME_LEN + SECTION_BYTECODE_HEADER_LEN) as u64)
        .unwrap();
    root.write_u64::<LittleEndian>(0).unwrap();
    root.write_u64::<LittleEndian>(1).unwrap();

    let err = BytecodeFile::<SectionNameContext>::from_bytes(binary_file_bytes(
        empty_context_bytes(),
        root,
    ))
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("bytecode extends past section end")
    );
}

#[test]
fn rejects_binary_instruction_stream_with_non_multiple_width() {
    let err = decode_instruction_stream::<TestInst>(&[0, 1, 2]).unwrap_err();

    assert!(err.to_string().contains("not a multiple"));
}

#[test]
fn parses_text_context_and_nested_sections() {
    let parsed = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n\
\t.header(root):\n\
\t\troot header\n\
\t.header(root).\n\
\t.text(root):\n\
\t\troot bytecode\n\
\t.text(root).\n\
\t.section(cpu):\n\
\t\t.header(cpu):\n\
\t\t\tcpu header\n\
\t\t.header(cpu).\n\
\t\t.text(cpu):\n\
\t\t\tcpu bytecode\n\
\t\t.text(cpu).\n\
\t\t.section(alu):\n\
\t\t\t.header(alu):\n\
\t\t\t\talu header\n\
\t\t\t.header(alu).\n\
\t\t\t.text(alu):\n\
\t\t\t\talu bytecode\n\
\t\t\t.text(alu).\n\
\t\t.section(alu).\n\
\t.section(cpu).\n\
.section(root).\n",
    ))
    .unwrap();

    assert!(parsed.context().section_names.is_empty());

    let root = parsed.root();
    assert!(root.path().is_root());
    assert_eq!(root.local_name(), None);
    assert_eq!(root.display_path().to_string(), "<root>");
    assert_eq!(root.header_text(), "\t\troot header\n");
    assert_eq!(root.sst(), "\t\troot bytecode\n");

    let cpu = root.child("cpu").unwrap();
    assert_eq!(path_components(cpu.path()), vec!["cpu"]);
    assert_eq!(cpu.local_name(), Some("cpu"));
    assert_eq!(cpu.display_path().to_string(), "cpu");
    assert_eq!(cpu.header_text(), "\t\t\tcpu header\n");
    assert_eq!(cpu.sst(), "\t\t\tcpu bytecode\n");

    let alu = cpu.child("alu").unwrap();
    assert_eq!(path_components(alu.path()), vec!["cpu", "alu"]);
    assert_eq!(alu.local_name(), Some("alu"));
    assert_eq!(alu.display_path().to_string(), "cpu/alu");
    assert_eq!(alu.header_text(), "\t\t\t\talu header\n");
    assert_eq!(alu.sst(), "\t\t\t\talu bytecode\n");
}

#[test]
fn parses_text_section_without_header_or_bytecode_as_empty_ranges() {
    let parsed =
        SstFile::<TextContext>::from_text(&text_file("", ".section(root):\n.section(root).\n"))
            .unwrap();

    let root = parsed.root();
    assert_eq!(root.header_text(), "");
    assert_eq!(root.sst(), "");
}

#[test]
fn parses_text_file_with_no_context() {
    let parsed: SstFile<NoContext> =
        SstFile::<NoContext>::from_text(&text_file("", ".section(root):\n.section(root).\n"))
            .unwrap();

    assert_eq!(parsed.context(), &NoContext);
    assert!(parsed.root().path().is_root());
}

#[test]
fn parses_text_file_with_no_context_and_no_global_section() {
    let parsed: SstFile<NoContext> =
        SstFile::<NoContext>::from_text("sst v1\n\n.section(root):\n.section(root).\n").unwrap();

    assert_eq!(parsed.context(), &NoContext);
    assert!(parsed.root().path().is_root());
}

#[test]
fn parses_text_file_with_empty_context_when_global_section_is_omitted() {
    let parsed: SstFile<TextContext> =
        SstFile::<TextContext>::from_text("sst v1\n\n.section(root):\n.section(root).\n").unwrap();

    assert!(parsed.context().section_names.is_empty());
    assert!(parsed.root().path().is_root());
}

#[test]
fn rejects_no_context_file_with_non_empty_global_section() {
    let err =
        SstFile::<NoContext>::from_text(&text_file("cpu\n", ".section(root):\n.section(root).\n"))
            .unwrap_err();

    assert!(
        err.to_string()
            .contains("NoContext accepts only an empty global section")
    );
}

#[test]
fn rejects_text_root_section_with_non_root_name() {
    let err =
        SstFile::<TextContext>::from_text(&text_file("", ".section(other):\n.section(other).\n"))
            .unwrap_err();

    assert!(
        err.to_string()
            .contains("root section must be named `root`")
    );
}

#[test]
fn rejects_text_bad_version() {
    let err = SstFile::<TextContext>::from_text(&format!(
        "sst v{}\n.global:\n.global.\n.section(root):\n.section(root).\n",
        VERSION + 1
    ))
    .unwrap_err();

    assert!(err.to_string().contains("unsupported sst version"));
}

#[test]
fn rejects_text_missing_context_end() {
    let err = SstFile::<TextContext>::from_text(
        "sst v1\n.global:\ncpu\n.section(root):\n.section(root).\n",
    )
    .unwrap_err();

    assert!(err.to_string().contains("unterminated context"));
}

#[test]
fn rejects_text_non_local_child_section_name() {
    let err = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n\t.section(gpu/core):\n\t.section(gpu/core).\n.section(root).\n",
    ))
    .unwrap_err();

    assert!(err.to_string().contains("must be a local name"));
}

#[test]
fn rejects_text_duplicate_child_sections() {
    let err = SstFile::<TextContext>::from_text(&text_file(
        "cpu\n",
        ".section(root):\n\t.section(cpu):\n\t.section(cpu).\n\t.section(cpu):\n\t.section(cpu).\n.section(root).\n",
    ))
    .unwrap_err();

    assert!(err.to_string().contains("duplicate child `cpu`"));
}

#[test]
fn rejects_text_mismatched_section_end_marker() {
    let err =
        SstFile::<TextContext>::from_text(&text_file("", ".section(root):\n.section(other).\n"))
            .unwrap_err();

    assert!(err.to_string().contains("mismatched marker `other`"));
}

#[test]
fn rejects_text_header_marker_with_wrong_section_name() {
    let err = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n\t.header(cpu):\n\t.header(cpu).\n.section(root).\n",
    ))
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("header marker for section `root` uses mismatched name `cpu`")
    );
}

#[test]
fn rejects_text_bytecode_end_marker_with_wrong_section_name() {
    let err = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n\t.text(root):\n\t.text(cpu).\n.section(root).\n",
    ))
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("bytecode marker for section `root` uses mismatched name `cpu`")
    );
}

#[test]
fn rejects_text_body_directly_inside_section() {
    let err = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n\tthis line is not in a header or bytecode block\n.section(root).\n",
    ))
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("unexpected content in section `<root>`")
    );
}

#[test]
fn parses_text_child_section_indented_with_spaces() {
    let parsed = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n  .section(cpu):\n  .section(cpu).\n.section(root).\n",
    ))
    .unwrap();

    assert!(parsed.root().child("cpu").is_some());
}

#[test]
fn parses_text_header_indented_with_spaces() {
    let parsed = SstFile::<TextContext>::from_text(&text_file(
        "",
        ".section(root):\n  .header(root):\n\t\troot header\n  .header(root).\n.section(root).\n",
    ))
    .unwrap();

    assert_eq!(parsed.root().header_text(), "\t\troot header\n");
}

fn binary_file_bytes(context: Vec<u8>, root: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.write_u16::<LittleEndian>(VERSION).unwrap();
    bytes.write_u16::<LittleEndian>(FLAGS).unwrap();
    bytes
        .write_u64::<LittleEndian>(context.len() as u64)
        .unwrap();
    bytes.extend_from_slice(&context);
    bytes.extend_from_slice(&root);
    bytes
}

fn context_bytes() -> Vec<u8> {
    context_with_strings(&["main", "cpu", "alu"])
}

fn empty_context_bytes() -> Vec<u8> {
    context_with_strings(&[])
}

fn context_with_strings(strings: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes
        .write_u32::<LittleEndian>(strings.len() as u32)
        .unwrap();
    for string in strings {
        write_string(&mut bytes, string);
    }
    bytes
}

fn binary_section_bytes(
    header: &[u8],
    instructions: &[TestInst],
    children: Vec<(u32, Vec<u8>)>,
) -> Vec<u8> {
    let mut bytecode = Vec::new();
    for inst in instructions {
        inst.write_bytes(&mut bytecode).unwrap();
    }
    let children = children
        .iter()
        .map(|(name_index, bytes)| (*name_index, bytes.as_slice()))
        .collect();
    raw_binary_section(header, &bytecode, children)
}

fn raw_binary_section(header: &[u8], bytecode: &[u8], children: Vec<(u32, &[u8])>) -> Vec<u8> {
    let child_table_len =
        CHILD_SECTION_TABLE_HEADER_LEN + children.len() * CHILD_SECTION_TABLE_ENTRY_LEN;
    let bytecode_start = SECTION_FRAME_LEN + header.len() + SECTION_BYTECODE_HEADER_LEN;
    let mut child_offset = bytecode_start + bytecode.len() + child_table_len;
    let section_len = child_offset + children.iter().map(|(_, child)| child.len()).sum::<usize>();

    let mut bytes = Vec::new();
    bytes.write_u64::<LittleEndian>(section_len as u64).unwrap();
    bytes
        .write_u64::<LittleEndian>(header.len() as u64)
        .unwrap();
    bytes.extend_from_slice(header);
    bytes
        .write_u64::<LittleEndian>(bytecode.len() as u64)
        .unwrap();
    bytes.extend_from_slice(bytecode);
    bytes
        .write_u32::<LittleEndian>(children.len() as u32)
        .unwrap();
    for (name_index, child) in &children {
        bytes.write_u32::<LittleEndian>(*name_index).unwrap();
        bytes
            .write_u64::<LittleEndian>(child_offset as u64)
            .unwrap();
        child_offset += child.len();
    }
    for (_, child) in children {
        bytes.extend_from_slice(child);
    }
    bytes
}

fn raw_binary_section_with_entry_offsets(
    header: &[u8],
    bytecode: &[u8],
    children: Vec<(u32, u64, &[u8])>,
) -> Vec<u8> {
    let child_table_len =
        CHILD_SECTION_TABLE_HEADER_LEN + children.len() * CHILD_SECTION_TABLE_ENTRY_LEN;
    let bytecode_start = SECTION_FRAME_LEN + header.len() + SECTION_BYTECODE_HEADER_LEN;
    let section_len = bytecode_start
        + bytecode.len()
        + child_table_len
        + children
            .iter()
            .map(|(_, _, child)| child.len())
            .sum::<usize>();

    let mut bytes = Vec::new();
    bytes.write_u64::<LittleEndian>(section_len as u64).unwrap();
    bytes
        .write_u64::<LittleEndian>(header.len() as u64)
        .unwrap();
    bytes.extend_from_slice(header);
    bytes
        .write_u64::<LittleEndian>(bytecode.len() as u64)
        .unwrap();
    bytes.extend_from_slice(bytecode);
    bytes
        .write_u32::<LittleEndian>(children.len() as u32)
        .unwrap();
    for (name_index, offset, _) in &children {
        bytes.write_u32::<LittleEndian>(*name_index).unwrap();
        bytes.write_u64::<LittleEndian>(*offset).unwrap();
    }
    for (_, _, child) in children {
        bytes.extend_from_slice(child);
    }
    bytes
}

fn path_components(path: &SectionPath) -> Vec<&str> {
    path.components().iter().map(String::as_str).collect()
}

fn text_file(context: &str, sections: &str) -> String {
    format!("sst v{VERSION}\n\n.global:\n{context}.global.\n\n{sections}")
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.write_u32::<LittleEndian>(value.len() as u32).unwrap();
    bytes.extend_from_slice(value.as_bytes());
}

fn read_string(bytes: &mut &[u8]) -> eyre::Result<String> {
    let len = bytes.read_u32::<LittleEndian>()? as usize;
    let mut value = vec![0; len];
    bytes.read_exact(&mut value)?;
    Ok(String::from_utf8(value)?)
}
