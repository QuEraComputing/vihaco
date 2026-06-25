// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::{io::Read, str::FromStr};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use vihaco::{
    binary::{FLAGS, MAGIC, VERSION},
    traits::{FromBytes, WriteBytes},
    BytecodeContext, BytecodeFile, ConstantId, Effects, GeneratedComponent, GetProgramGlobal,
    Instruction, LoadInput, LoadSection, ProgramLoader, Value,
};

const CHILD_NAME: u32 = 0;
const DEFAULT_CHILD_NAME: u32 = 1;
const EXTRA_NAME: u32 = 2;
const MIDDLE_NAME: u32 = 3;
const LEAF_NAME: u32 = 4;
const SECTION_FRAME_LEN: usize = 8 + 8;
const SECTION_BYTECODE_HEADER_LEN: usize = 8;
const CHILD_SECTION_TABLE_HEADER_LEN: usize = 4;
const CHILD_SECTION_TABLE_ENTRY_LEN: usize = 4 + 8;

#[derive(Debug, Clone, PartialEq, Instruction)]
enum TestInst {
    Nop,
    Load(ConstantId),
}

#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser::Parse)]
enum TextInst {
    Nop,
    Alt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TestHeader {
    cores: u32,
}

impl FromBytes for TestHeader {
    fn from_bytes<R: Read>(bytes: &mut R) -> eyre::Result<Self> {
        Ok(Self {
            cores: bytes.read_u32::<LittleEndian>()?,
        })
    }
}

impl WriteBytes for TestHeader {
    fn write_bytes<W: std::io::Write>(&self, io: &mut W) -> eyre::Result<()> {
        io.write_u32::<LittleEndian>(self.cores)?;
        Ok(())
    }
}

impl FromStr for TestHeader {
    type Err = std::num::ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            cores: text.parse()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextContext {
    section_names: Vec<String>,
}

impl BytecodeContext for TextContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let text = std::str::from_utf8(bytes)?;
        Ok(Self {
            section_names: text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        })
    }

    fn section_name(&self, index: u32) -> Option<&str> {
        self.section_names.get(index as usize).map(String::as_str)
    }
}

#[derive(Debug, Clone, Default)]
struct LoadedDevice {
    program: ProgramLoader<TestInst>,
}

#[derive(Debug, Clone, Default)]
struct TextLoadedDevice {
    program: ProgramLoader<TextInst, TextContext>,
}

impl GeneratedComponent for LoadedDevice {
    type Instruction = TestInst;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

impl GeneratedComponent for TextLoadedDevice {
    type Instruction = TextInst;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

impl LoadSection for LoadedDevice {
    fn load_section<'bc>(&mut self, input: LoadInput<'bc>) -> eyre::Result<()> {
        self.program.load_section(input)
    }
}

impl LoadSection<String, TextContext> for TextLoadedDevice {
    fn load_section<'bc>(
        &mut self,
        input: LoadInput<'bc, String, TextContext>,
    ) -> eyre::Result<()> {
        self.program.load_section(input)
    }
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct Machine {
    #[program]
    program: ProgramLoader<TestInst>,

    #[device(0x01)]
    #[loadable("child")]
    child: LoadedDevice,

    #[device(0x02)]
    #[loadable]
    default_child: LoadedDevice,
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct NestedMachine {
    #[program]
    program: ProgramLoader<TestInst>,

    #[device(0x01)]
    #[loadable("leaf")]
    leaf: LoadedDevice,
}

impl GeneratedComponent for NestedMachine {
    type Instruction = NestedMachineInstruction;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct HostMachine {
    #[program]
    program: ProgramLoader<TestInst>,

    #[device(0x01)]
    #[loadable("middle")]
    middle: NestedMachine,
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct HeaderMachine {
    #[header]
    info: TestHeader,

    #[program]
    program: ProgramLoader<TestInst>,

    #[device(0x01)]
    device: LoadedDevice,
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct TextMachine {
    #[program]
    program: ProgramLoader<TextInst, TextContext>,

    #[device(0x01)]
    #[loadable("child")]
    child: TextLoadedDevice,

    #[device(0x02)]
    #[loadable]
    default_child: TextLoadedDevice,
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct TextNestedMachine {
    #[program]
    program: ProgramLoader<TextInst, TextContext>,

    #[device(0x01)]
    #[loadable("leaf")]
    leaf: TextLoadedDevice,
}

impl GeneratedComponent for TextNestedMachine {
    type Instruction = TextNestedMachineInstruction;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct TextHostMachine {
    #[program]
    program: ProgramLoader<TextInst, TextContext>,

    #[device(0x01)]
    #[loadable("middle")]
    middle: TextNestedMachine,
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct TextHeaderMachine {
    #[header]
    info: TestHeader,

    #[program]
    program: ProgramLoader<TextInst, TextContext>,

    #[device(0x01)]
    device: TextLoadedDevice,
}

#[test]
fn binary_generated_loadable_routes_program_and_child_sections() {
    let child = binary_section_bytes(b"", &[TestInst::Load(ConstantId(0))], vec![]);
    let default_child = binary_section_bytes(b"", &[TestInst::Nop], vec![]);
    let root = binary_section_bytes(
        b"",
        &[TestInst::Nop],
        vec![(CHILD_NAME, child), (DEFAULT_CHILD_NAME, default_child)],
    );
    let file: BytecodeFile<Vec<u8>> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();

    let mut machine = Machine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.program.code, vec![TestInst::Nop]);
    assert_eq!(
        machine.child.program.code,
        vec![TestInst::Load(ConstantId(0))]
    );
    assert_eq!(machine.default_child.program.code, vec![TestInst::Nop]);
    assert!(machine
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .child
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert_eq!(
        machine.program.get_constant(ConstantId(0)).unwrap(),
        &Value::I64(9)
    );
}

#[test]
fn text_generated_loadable_routes_program_and_child_sections() {
    let file = text_file(
        &["child", "default_child"],
        "~> /:\n\
\t^>\n\
\t\tnop\n\
\t<^\n\
\t~> child:\n\
\t\t^>\n\
\t\t\talt\n\
\t\t<^\n\
\t<~ child.\n\
\t~> default_child:\n\
\t\t^>\n\
\t\t\tnop\n\
\t\t<^\n\
\t<~ default_child.\n\
<~ /.\n",
    );

    let mut machine = TextMachine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.program.code, vec![TextInst::Nop]);
    assert_eq!(machine.child.program.code, vec![TextInst::Alt]);
    assert_eq!(machine.default_child.program.code, vec![TextInst::Nop]);
    assert!(machine
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .child
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
}

#[test]
fn binary_generated_loadable_parses_marked_header() {
    let mut header = Vec::new();
    TestHeader { cores: 8 }.write_bytes(&mut header).unwrap();
    let root = binary_section_bytes(&header, &[TestInst::Nop], vec![]);
    let file: BytecodeFile<Vec<u8>> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();

    let mut machine = HeaderMachine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.info, TestHeader { cores: 8 });
    assert_eq!(machine.program.code, vec![TestInst::Nop]);
}

#[test]
fn text_generated_loadable_parses_marked_header() {
    let file = text_file(
        &[],
        "~> /:\n\
\t!>\n\
\t\t8\n\
\t<!\n\
\t^>\n\
\t\tnop\n\
\t<^\n\
<~ /.\n",
    );

    let mut machine = TextHeaderMachine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.info, TestHeader { cores: 8 });
    assert_eq!(machine.program.code, vec![TextInst::Nop]);
}

#[test]
fn binary_generated_loadable_routes_three_level_section_tree() {
    let leaf = binary_section_bytes(b"", &[TestInst::Nop, TestInst::Load(ConstantId(0))], vec![]);
    let middle = binary_section_bytes(
        b"",
        &[TestInst::Load(ConstantId(0))],
        vec![(LEAF_NAME, leaf)],
    );
    let root = binary_section_bytes(b"", &[TestInst::Nop], vec![(MIDDLE_NAME, middle)]);
    let file: BytecodeFile<Vec<u8>> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();

    let mut machine = HostMachine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.program.code, vec![TestInst::Nop]);
    assert_eq!(
        machine.middle.program.code,
        vec![TestInst::Load(ConstantId(0))]
    );
    assert_eq!(
        machine.middle.leaf.program.code,
        vec![TestInst::Nop, TestInst::Load(ConstantId(0))]
    );
    assert!(machine
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .middle
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .middle
        .leaf
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
}

#[test]
fn text_generated_loadable_routes_three_level_section_tree() {
    let file = text_file(
        &["middle", "leaf"],
        "~> /:\n\
\t^>\n\
\t\tnop\n\
\t<^\n\
\t~> middle:\n\
\t\t^>\n\
\t\t\talt\n\
\t\t<^\n\
\t\t~> leaf:\n\
\t\t\t^>\n\
\t\t\t\tnop\n\
\t\t\t\talt\n\
\t\t\t<^\n\
\t\t<~ leaf.\n\
\t<~ middle.\n\
<~ /.\n",
    );

    let mut machine = TextHostMachine::default();
    machine.load_section(LoadInput::from(&file)).unwrap();

    assert_eq!(machine.program.code, vec![TextInst::Nop]);
    assert_eq!(machine.middle.program.code, vec![TextInst::Alt]);
    assert_eq!(
        machine.middle.leaf.program.code,
        vec![TextInst::Nop, TextInst::Alt]
    );
    assert!(machine
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .middle
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
    assert!(machine
        .middle
        .leaf
        .program
        .context
        .as_ref()
        .unwrap()
        .ptr_eq(&file.context_handle()));
}

#[test]
fn binary_generated_loadable_requires_marked_children() {
    let root = binary_section_bytes(b"", &[TestInst::Nop], vec![]);
    let file: BytecodeFile<Vec<u8>> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();
    let mut machine = Machine::default();

    let err = machine.load_section(LoadInput::from(&file)).unwrap_err();

    assert!(err.to_string().contains("missing required child section"));
}

#[test]
fn text_generated_loadable_requires_marked_children() {
    let file = text_file(
        &["child", "default_child"],
        "~> /:\n\
\t^>\n\
\t\tnop\n\
\t<^\n\
<~ /.\n",
    );
    let mut machine = TextMachine::default();

    let err = machine.load_section(LoadInput::from(&file)).unwrap_err();

    assert!(err.to_string().contains("missing required child section"));
}

#[test]
fn binary_generated_loadable_rejects_unexpected_direct_children() {
    let extra = binary_section_bytes(b"", &[], vec![]);
    let root = binary_section_bytes(b"", &[TestInst::Nop], vec![(EXTRA_NAME, extra)]);
    let file: BytecodeFile<Vec<u8>> =
        BytecodeFile::from_bytes(binary_file_bytes(context_bytes(), root)).unwrap();
    let mut machine = Machine::default();

    let err = machine.load_section(LoadInput::from(&file)).unwrap_err();

    assert!(err.to_string().contains("unexpected child section"));
}

#[test]
fn text_generated_loadable_rejects_unexpected_direct_children() {
    let file = text_file(
        &["child", "default_child", "extra"],
        "~> /:\n\
\t^>\n\
\t\tnop\n\
\t<^\n\
\t~> child:\n\
\t<~ child.\n\
\t~> default_child:\n\
\t<~ default_child.\n\
\t~> extra:\n\
\t<~ extra.\n\
<~ /.\n",
    );
    let mut machine = TextMachine::default();

    let err = machine.load_section(LoadInput::from(&file)).unwrap_err();

    assert!(err.to_string().contains("unexpected child section"));
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

fn text_file(section_names: &[&str], sections: &str) -> BytecodeFile<String, TextContext> {
    let context = section_names.join("\n");
    let context = if context.is_empty() {
        String::new()
    } else {
        format!("{context}\n")
    };
    BytecodeFile::<String, TextContext>::from_text(&format!(
        "vhbc{VERSION}\n\n@>\n{context}<@\n\n{sections}"
    ))
    .unwrap()
}

fn context_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.write_u32::<LittleEndian>(1).unwrap();
    Value::I64(9).write_bytes(&mut bytes).unwrap();

    bytes.write_u32::<LittleEndian>(5).unwrap();
    write_string(&mut bytes, "child");
    write_string(&mut bytes, "default_child");
    write_string(&mut bytes, "extra");
    write_string(&mut bytes, "middle");
    write_string(&mut bytes, "leaf");
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
    bytes.extend_from_slice(&bytecode);
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
        bytes.extend_from_slice(&child);
    }
    bytes
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.write_u32::<LittleEndian>(value.len() as u32).unwrap();
    bytes.extend_from_slice(value.as_bytes());
}
