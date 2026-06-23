// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::format::{
    ChildSectionTableEntry, ChildSectionTableHeader, SectionBytecodeHeader, SectionFrame,
};
use super::*;
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

#[test]
fn parses_context_and_nested_sections() {
    const CPU_NAME: u32 = 1;
    const ALU_NAME: u32 = 2;

    let context = context_bytes();
    let alu_header = b"alu header";
    let cpu_header = b"cpu header";
    let root_header = b"root header";
    let alu = section_bytes(alu_header, &[TestInst::Nop], vec![]);
    let cpu = section_bytes(
        cpu_header,
        &[TestInst::Load(ConstantId(0))],
        vec![(ALU_NAME, alu)],
    );
    let root = section_bytes(root_header, &[], vec![(CPU_NAME, cpu)]);
    let file = file_bytes(context, root);

    let parsed: BytecodeFile = BytecodeFile::from_bytes(file).unwrap();

    assert_eq!(parsed.context().constants, vec![Value::I64(42)]);
    assert_eq!(
        parsed.context().strings,
        vec!["main".to_string(), "cpu".to_string(), "alu".to_string()]
    );
    assert_eq!(parsed.context().main_function, Some(0));
    assert_eq!(parsed.context().file, 7);

    let root = parsed.root();
    assert!(root.path().is_root());
    assert_eq!(root.path().components(), &[]);
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
fn parses_file_with_custom_context_representation() {
    const CPU_NAME: u32 = 1;

    let cpu = section_bytes(b"", &[TestInst::Nop], vec![]);
    let root = section_bytes(b"", &[], vec![(CPU_NAME, cpu)]);
    let parsed: BytecodeFile<WrappedContext> =
        BytecodeFile::from_bytes(file_bytes(context_bytes(), root)).unwrap();

    assert_eq!(parsed.context().inner.constants, vec![Value::I64(42)]);
    assert_eq!(
        parsed.root().child("cpu").unwrap().local_name(),
        Some("cpu")
    );
}

#[test]
fn walks_section_tree_depth_first() {
    const CPU_NAME: u32 = 0;
    const ALU_NAME: u32 = 1;
    const GPU_NAME: u32 = 2;

    let alu = section_bytes(b"", &[TestInst::Load(ConstantId(0))], vec![]);
    let cpu = section_bytes(b"", &[TestInst::Nop], vec![(ALU_NAME, alu)]);
    let gpu = section_bytes(b"", &[], vec![]);
    let root = section_bytes(b"", &[], vec![(CPU_NAME, cpu), (GPU_NAME, gpu)]);
    let parsed: BytecodeFile = BytecodeFile::from_bytes(file_bytes(
        context_with_strings(&["cpu", "alu", "gpu"]),
        root,
    ))
    .unwrap();

    let paths = parsed
        .sections()
        .map(|section| section.display_path().to_string())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec!["<root>", "cpu", "cpu/alu", "gpu"]);

    let descendant_paths = parsed
        .root()
        .descendants()
        .map(|section| section.display_path().to_string())
        .collect::<Vec<_>>();

    assert_eq!(descendant_paths, vec!["cpu", "cpu/alu", "gpu"]);
}

#[test]
fn parses_context_with_custom_value_and_type_tables() {
    let context = custom_context_bytes();
    let root = section_bytes(b"", &[], vec![]);
    let parsed = BytecodeFile::<ProgramContext<CustomValue, CustomType>>::from_bytes(file_bytes(
        context, root,
    ))
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
fn parse_header_consumes_the_whole_header() {
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
    let root = section_bytes(&header, &[], vec![]);
    let parsed: BytecodeFile =
        BytecodeFile::from_bytes(file_bytes(empty_context_bytes(), root)).unwrap();

    assert_eq!(parsed.root().parse_header::<Header>().unwrap(), Header(99));
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = file_bytes(empty_context_bytes(), section_bytes(b"", &[], vec![]));
    bytes[0] = b'X';

    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(bytes).unwrap_err();
    assert!(err.to_string().contains("invalid bytecode magic"));
}

#[test]
fn rejects_missing_section_name_string() {
    let root = raw_section(b"", b"", vec![(0, b"")]);
    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(file_bytes(
        empty_context_bytes(),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("missing section name string"));
}

#[test]
fn rejects_duplicate_child_names() {
    let child_a = section_bytes(b"", &[], vec![]);
    let child_b = section_bytes(b"", &[], vec![]);
    let root = raw_section(b"", b"", vec![(0, &child_a), (0, &child_b)]);

    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("duplicate child"));
}

#[test]
fn rejects_out_of_bounds_child_section() {
    let root = raw_section_with_entry_offsets(b"", b"", vec![(0, 999, b"")]);

    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(file_bytes(
        context_with_strings(&["cpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("extends past section end"));
}

#[test]
fn rejects_overlapping_child_sections() {
    let child = section_bytes(b"", &[], vec![]);
    let child_offset = (SectionFrame::ENCODED_LEN
        + SectionBytecodeHeader::ENCODED_LEN
        + ChildSectionTableHeader::ENCODED_LEN
        + (2 * ChildSectionTableEntry::ENCODED_LEN)) as u64;
    let root = raw_section_with_entry_offsets(
        b"",
        b"",
        vec![(0, child_offset, &child), (1, child_offset, &[])],
    );

    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(file_bytes(
        context_with_strings(&["cpu", "gpu"]),
        root,
    ))
    .unwrap_err();

    assert!(err.to_string().contains("overlaps"));
}

#[test]
fn rejects_bytecode_that_extends_past_section_end() {
    let mut root = Vec::new();
    root.write_u64::<LittleEndian>(
        (SectionFrame::ENCODED_LEN + SectionBytecodeHeader::ENCODED_LEN) as u64,
    )
    .unwrap();
    root.write_u64::<LittleEndian>(0).unwrap();
    root.write_u64::<LittleEndian>(1).unwrap();

    let err = BytecodeFile::<ProgramContext<Value, Type>>::from_bytes(file_bytes(
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
fn rejects_non_multiple_instruction_stream() {
    let err = decode_instruction_stream::<TestInst>(&[0, 1, 2]).unwrap_err();

    assert!(err.to_string().contains("not a multiple"));
}

fn file_bytes(context: Vec<u8>, root: Vec<u8>) -> Vec<u8> {
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

fn section_bytes(
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
    raw_section(header, &bytecode, children)
}

fn raw_section(header: &[u8], bytecode: &[u8], children: Vec<(u32, &[u8])>) -> Vec<u8> {
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

fn raw_section_with_entry_offsets(
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

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.write_u32::<LittleEndian>(value.len() as u32).unwrap();
    bytes.extend_from_slice(value.as_bytes());
}
