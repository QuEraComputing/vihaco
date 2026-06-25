// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::format::{
    ChildSectionTableEntry, ChildSectionTableHeader, SectionBytecodeHeader, SectionFrame,
};
use super::*;
use crate::binary::file::{BinaryBytecodeFile, TextBytecodeFile};
use crate::{
    traits::{FromBytes, WriteBytes},
    value::{Type, Value},
};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Read;

#[derive(Debug, Clone, PartialEq, crate::Instruction)]
enum TestInst {
    Nop,
    Load(ConstantId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CustomValue(u8);

impl FromBytes for CustomValue {
    fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
        Ok(Self(bytes.read_u8()?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CustomType(u8);

impl FromBytes for CustomType {
    fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
        Ok(Self(bytes.read_u8()?))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct WrappedContext {
    inner: ProgramContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextContext {
    raw: String,
    section_names: Vec<String>,
}

impl BytecodeContext for WrappedContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        Ok(Self {
            inner: ProgramContext::from_bytes(bytes)?,
        })
    }

    fn section_name(&self, index: u32) -> Option<&str> {
        self.inner.section_name(index)
    }
}

impl BytecodeContext for TextContext {
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

    fn section_name(&self, index: u32) -> Option<&str> {
        self.section_names.get(index as usize).map(String::as_str)
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

    let parsed: BinaryBytecodeFile = BinaryBytecodeFile::from_bytes(file).unwrap();

    assert_eq!(parsed.context().constants, vec![Value::I64(42)]);
    assert_eq!(
        parsed.context().strings,
        vec!["main".to_string(), "cpu".to_string(), "alu".to_string()]
    );
    assert_eq!(parsed.context().main_function, Some(0));
    assert_eq!(parsed.context().file, 7);

    let root = parsed.root();
    assert!(root.path().is_root());
    assert_eq!(root.path().components(), &[] as &[u32]);
    assert_eq!(root.local_name(), None);
    assert_eq!(root.display_path().to_string(), "<root>");
    assert_eq!(root.header_bytes(), b"root header");

    let cpu = root.child("cpu").unwrap();
    assert_eq!(cpu.path().components(), &[CPU_NAME]);
    assert_eq!(cpu.local_name(), Some("cpu"));
    assert_eq!(cpu.display_path().to_string(), "cpu");
    assert_eq!(cpu.header_bytes(), b"cpu header");
    assert_eq!(
        decode_instruction_stream::<TestInst>(cpu.bytecode()).unwrap(),
        vec![TestInst::Load(ConstantId(0))]
    );

    let alu = cpu.child("alu").unwrap();
    assert_eq!(alu.path().components(), &[CPU_NAME, ALU_NAME]);
    assert_eq!(alu.local_name(), Some("alu"));
    assert_eq!(alu.display_path().to_string(), "cpu/alu");
    assert_eq!(alu.header_bytes(), b"alu header");
    assert_eq!(
        decode_instruction_stream::<TestInst>(alu.bytecode()).unwrap(),
        vec![TestInst::Nop]
    );
}

#[test]
fn parses_binary_file_with_custom_context_representation() {
    const CPU_NAME: u32 = 1;

    let cpu = binary_section_bytes(b"", &[TestInst::Nop], vec![]);
    let root = binary_section_bytes(b"", &[], vec![(CPU_NAME, cpu)]);
    let parsed: BinaryBytecodeFile<WrappedContext> =
        BinaryBytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();

    assert_eq!(parsed.context().inner.constants, vec![Value::I64(42)]);
    assert_eq!(
        parsed.root().child("cpu").unwrap().local_name(),
        Some("cpu")
    );
}

#[test]
fn parses_binary_context_with_custom_value_and_type_tables() {
    let context = custom_context_bytes();
    let root = binary_section_bytes(b"", &[], vec![]);
    let parsed = BinaryBytecodeFile::<ProgramContext<CustomValue, CustomType>>::from_bytes(
        binary_file_bytes(context, root),
    )
    .unwrap();

    assert_eq!(parsed.context().constants, vec![CustomValue(7)]);
    assert_eq!(parsed.context().functions.len(), 1);
    assert_eq!(
        parsed.context().functions[0].signature.params[0].ty,
        CustomType(3)
    );
    assert_eq!(
        parsed.context().functions[0].signature.ret,
        vec![CustomType(4)]
    );
}

#[test]
fn binary_parse_header_consumes_the_whole_header() {
    #[derive(Debug, PartialEq)]
    struct Header(u32);

    impl FromBytes for Header {
        fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
            Ok(Header(bytes.read_u32::<LittleEndian>()?))
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
    let parsed: BinaryBytecodeFile =
        BinaryBytecodeFile::from_bytes(binary_file_bytes(empty_context_bytes(), root)).unwrap();

    assert_eq!(parsed.root().parse_header::<Header>().unwrap(), Header(99));
}

#[test]
fn rejects_bad_binary_magic() {
    let mut bytes = binary_file_bytes(
        empty_context_bytes(),
        binary_section_bytes(b"", &[], vec![]),
    );
    bytes[0] = b'X';

    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(bytes).unwrap_err();
    assert!(err.to_string().contains("invalid bytecode magic"));
}

#[test]
fn rejects_binary_missing_section_name_string() {
    let root = raw_binary_section(b"", b"", vec![(0, b"")]);
    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(binary_file_bytes(
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

    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("duplicate child"));
}

#[test]
fn rejects_binary_out_of_bounds_child_section() {
    let root = raw_binary_section_with_entry_offsets(b"", b"", vec![(0, 999, b"")]);

    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("extends past section end"));
}

#[test]
fn rejects_binary_overlapping_child_sections() {
    let child = binary_section_bytes(b"", &[], vec![]);
    let child_offset = (SectionFrame::ENCODED_LEN
        + SectionBytecodeHeader::ENCODED_LEN
        + ChildSectionTableHeader::ENCODED_LEN
        + (2 * ChildSectionTableEntry::ENCODED_LEN)) as u64;
    let root = raw_binary_section_with_entry_offsets(
        b"",
        b"",
        vec![(0, child_offset, &child), (1, child_offset, &[])],
    );

    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(binary_file_bytes(
        context_with_strings(&["cpu", "gpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("overlaps"));
}

#[test]
fn rejects_binary_bytecode_that_extends_past_section_end() {
    let mut root = Vec::new();
    root.write_u64::<LittleEndian>(
        (SectionFrame::ENCODED_LEN + SectionBytecodeHeader::ENCODED_LEN) as u64,
    )
    .unwrap();
    root.write_u64::<LittleEndian>(0).unwrap();
    root.write_u64::<LittleEndian>(1).unwrap();

    let err = BinaryBytecodeFile::<ProgramContext<Value, Type>>::from_bytes(binary_file_bytes(
        empty_context_bytes(),
        root,
    ))
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("bytecode extends past section end"));
}

#[test]
fn rejects_binary_instruction_stream_with_non_multiple_width() {
    let err = decode_instruction_stream::<TestInst>(&[0, 1, 2]).unwrap_err();

    assert!(err.to_string().contains("not a multiple"));
}

#[test]
fn parses_text_context_and_nested_sections() {
    let parsed = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "cpu\nalu\n",
        r#"begin section root:
begin header root:
root header
end header root
begin bytecode root:
root bytecode
end bytecode root
begin section cpu:
begin header cpu:
cpu header
end header cpu
begin bytecode cpu:
cpu bytecode
end bytecode cpu
begin section alu:
begin header alu:
alu header
end header alu
begin bytecode alu:
alu bytecode
end bytecode alu
end section alu
end section cpu
end section root
"#,
    ))
    .unwrap();

    assert_eq!(parsed.context().section_names, vec!["cpu", "alu"]);

    let root = parsed.root();
    assert!(root.path().is_root());
    assert_eq!(root.local_name(), None);
    assert_eq!(root.display_path().to_string(), "<root>");
    assert_eq!(root.header_text(), "root header\n");
    assert_eq!(root.text(), "root bytecode\n");

    let cpu = root.child("cpu").unwrap();
    assert_eq!(cpu.path().components(), &[0]);
    assert_eq!(cpu.local_name(), Some("cpu"));
    assert_eq!(cpu.display_path().to_string(), "cpu");
    assert_eq!(cpu.header_text(), "cpu header\n");
    assert_eq!(cpu.text(), "cpu bytecode\n");

    let alu = cpu.child("alu").unwrap();
    assert_eq!(alu.path().components(), &[0, 1]);
    assert_eq!(alu.local_name(), Some("alu"));
    assert_eq!(alu.display_path().to_string(), "cpu/alu");
    assert_eq!(alu.header_text(), "alu header\n");
    assert_eq!(alu.text(), "alu bytecode\n");
}

#[test]
fn parses_text_section_without_header_or_bytecode_as_empty_ranges() {
    let parsed = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "",
        r#"begin section root:
end section root
"#,
    ))
    .unwrap();

    let root = parsed.root();
    assert_eq!(root.header_text(), "");
    assert_eq!(root.text(), "");
}

#[test]
fn rejects_text_bad_version() {
    let err = TextBytecodeFile::<TextContext>::from_text(&format!(
        "vihaco version {}\nbegin context:\nend context\nbegin section root:\nend section root\n",
        VERSION + 1
    ))
    .unwrap_err();

    assert!(err.to_string().contains("unsupported bytecode version"));
}

#[test]
fn rejects_text_missing_context_end() {
    let err = TextBytecodeFile::<TextContext>::from_text(
        "vihaco version 1\nbegin context:\ncpu\nbegin section root:\nend section root\n",
    )
    .unwrap_err();

    assert!(err.to_string().contains("unterminated context"));
}

#[test]
fn rejects_text_missing_section_name() {
    let err = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "cpu\n",
        r#"begin section root:
begin section gpu:
end section gpu
end section root
"#,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("missing section name `gpu`"));
}

#[test]
fn rejects_text_duplicate_child_sections() {
    let err = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "cpu\n",
        r#"begin section root:
begin section cpu:
end section cpu
begin section cpu:
end section cpu
end section root
"#,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("duplicate child `cpu`"));
}

#[test]
fn rejects_text_mismatched_section_end_marker() {
    let err = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "",
        r#"begin section root:
end section other
"#,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("mismatched marker `other`"));
}

#[test]
fn rejects_text_body_directly_inside_section() {
    let err = TextBytecodeFile::<TextContext>::from_text(&text_file(
        "",
        r#"begin section root:
this line is not in a header or bytecode block
end section root
"#,
    ))
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("unexpected content in section `<root>`"));
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
    let mut bytes = Vec::new();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    Value::I64(42).write_bytes(&mut bytes).unwrap();

    bytes.write_u32::<LittleEndian>(3).unwrap();
    write_string(&mut bytes, "main");
    write_string(&mut bytes, "cpu");
    write_string(&mut bytes, "alu");

    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(1).unwrap();
    Type::I64.write_bytes(&mut bytes).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(7).unwrap();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();

    bytes.write_u8(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(7).unwrap();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    write_string(&mut bytes, "cpu");

    bytes
}

fn custom_context_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u8(7).unwrap();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    write_string(&mut bytes, "main");

    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u8(3).unwrap();
    bytes.write_u32::<LittleEndian>(1).unwrap();
    bytes.write_u8(4).unwrap();
    bytes.write_u32::<LittleEndian>(2).unwrap();
    bytes.write_u32::<LittleEndian>(11).unwrap();
    bytes.write_u32::<LittleEndian>(22).unwrap();
    bytes.write_u32::<LittleEndian>(5).unwrap();

    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u8(1).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(5).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();

    bytes
}

fn empty_context_bytes() -> Vec<u8> {
    context_with_strings(&[])
}

fn context_with_strings(strings: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes
        .write_u32::<LittleEndian>(strings.len() as u32)
        .unwrap();
    for string in strings {
        write_string(&mut bytes, string);
    }
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u8(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
    bytes.write_u32::<LittleEndian>(0).unwrap();
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
        ChildSectionTableHeader::ENCODED_LEN + children.len() * ChildSectionTableEntry::ENCODED_LEN;
    let bytecode_start =
        SectionFrame::ENCODED_LEN + header.len() + SectionBytecodeHeader::ENCODED_LEN;
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
        ChildSectionTableHeader::ENCODED_LEN + children.len() * ChildSectionTableEntry::ENCODED_LEN;
    let bytecode_start =
        SectionFrame::ENCODED_LEN + header.len() + SectionBytecodeHeader::ENCODED_LEN;
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

fn text_file(context: &str, sections: &str) -> String {
    format!("vihaco version {VERSION}\nbegin context:\n{context}end context\n{sections}")
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.write_u32::<LittleEndian>(value.len() as u32).unwrap();
    bytes.extend_from_slice(value.as_bytes());
}
