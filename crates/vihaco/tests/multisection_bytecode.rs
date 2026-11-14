// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(not(feature = "typed-declarations"))]

use std::{io::Read, str::FromStr};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use vihaco::{
    BytecodeFile, BytecodeGlobalContext, BytecodeSectionView, ConstantId, Effects, FLAGS,
    GeneratedComponent, GetProgramInfo, Instruction, LoadBytecodeSection, LoadSSTSection, MAGIC,
    ProgramImage, SST_VERSION, SSTFile, SSTGlobalContext, SSTHeader, SSTSectionView,
    SectionNameResolver, Type, VERSION, Value,
    module::LocalModule,
    syntax::{ParsedModule, Resolve},
    traits::{FromBytes, FromText, WriteBytes},
};

const CONTROLLER_NAME: u32 = 0;
const READOUT_NAME: u32 = 1;

#[derive(Debug, Clone, PartialEq, Instruction)]
enum TestInst {
    Nop,
    Load(ConstantId),
}

#[derive(Debug, Clone, PartialEq, Instruction, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "test")]
enum TextInst {
    Nop,
    Alt,
}

#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(instruction, head = "surface")]
enum SurfaceOnlyInst {
    Nop,
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

impl FromText for TestHeader {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(text.trim().parse()?)
    }
}

impl SSTHeader for TestHeader {}

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
            cores: text.trim().parse()?,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NoHeader;

impl FromText for NoHeader {
    fn from_text(_text: &str) -> eyre::Result<Self> {
        Ok(Self)
    }
}

impl SSTHeader for NoHeader {}

#[derive(Debug, Default)]
struct TextResolver;

impl<H> Resolve<TextInst, H> for TextResolver {
    type Module = LocalModule<TextInst, Value, Type>;

    fn resolve_module(&mut self, parsed: ParsedModule<TextInst, H>) -> eyre::Result<Self::Module> {
        Ok(LocalModule {
            code: parsed.code,
            symbols: parsed.symbols,
            ..LocalModule::default()
        })
    }
}

type BytecodeProgram = ProgramImage<TestInst, TextContext>;
type TextProgram = vihaco::loader::ProgramImage<TextInst, TextContext>;

fn load_bytecode_program<'bc>(
    program: &mut BytecodeProgram,
    section: BytecodeSectionView<'bc, TextContext>,
) -> eyre::Result<()> {
    program.module.code = section.decode_instructions()?;
    program.module.constants = vec![Value::I64(9)];
    program.context = Some(section.context_handle());
    program.pc = 0;
    Ok(())
}

fn load_parsed_text_program<H>(
    program: &mut TextProgram,
    parsed: ParsedModule<TextInst, H>,
    context: vihaco::ContextHandle<TextContext>,
) -> eyre::Result<()> {
    let mut resolver = TextResolver;
    program.module = resolver.resolve_module(parsed)?;
    program.context = Some(context);
    program.pc = 0;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextContext {
    section_names: Vec<String>,
}

impl SectionNameResolver for TextContext {
    fn section_name(&self, index: u32) -> Option<&str> {
        self.section_names.get(index as usize).map(String::as_str)
    }
}

impl BytecodeGlobalContext for TextContext {
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let text = std::str::from_utf8(bytes)?;
        <Self as SSTGlobalContext>::from_text(text)
    }
}

impl SSTGlobalContext for TextContext {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(Self {
            section_names: text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        })
    }
}

#[derive(Debug, Clone, Default)]
struct LoadedDevice {
    program: BytecodeProgram,
}

#[derive(Debug, Clone, Default)]
struct TextLoadedDevice {
    program: TextProgram,
}

#[derive(Debug, Clone, PartialEq, vihaco::Parse)]
#[syntax_class(instruction, head = "legacy")]
enum LegacySyntax {
    #[pattern = "'noop"]
    Noop,
}

impl GeneratedComponent for LoadedDevice {
    type Instruction = TestInst;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: &Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

impl vihaco::HasInstructionSet for LoadedDevice {
    type Runtime = TestInst;
    type Syntax = LegacySyntax;
}

impl vihaco::Component for LoadedDevice {}

impl GeneratedComponent for TextLoadedDevice {
    type Instruction = TextInst;
    type Message = ();
    type Effect = ();

    fn execute_generated(
        &mut self,
        _inst: &Self::Instruction,
        _msg: Self::Message,
    ) -> eyre::Result<Effects<Self::Effect>> {
        Ok(Effects::none())
    }
}

impl vihaco::HasInstructionSet for TextLoadedDevice {
    type Runtime = TextInst;
    type Syntax = LegacySyntax;
}

impl vihaco::Component for TextLoadedDevice {}

impl LoadBytecodeSection<TextContext> for LoadedDevice {
    type Output = ();

    fn load_bytecode_section<'bc>(
        &mut self,
        section: BytecodeSectionView<'bc, TextContext>,
    ) -> eyre::Result<()> {
        load_bytecode_program(&mut self.program, section)
    }
}

impl LoadSSTSection<TextContext> for TextLoadedDevice {
    type Output = ();

    fn load_sst_section<'bc>(
        &mut self,
        section: SSTSectionView<'bc, TextContext>,
    ) -> eyre::Result<()> {
        let parsed = ParsedModule::<TextInst, NoHeader>::parse_section(section.clone())?;
        load_parsed_text_program(&mut self.program, parsed, section.context_handle())?;
        Ok(())
    }
}

/// A composite loads its own section directly; there is no generated routing.
#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct HeaderMachine {
    info: TestHeader,

    program: BytecodeProgram,

    #[device(0x01)]
    device: LoadedDevice,
}

impl LoadBytecodeSection<TextContext> for HeaderMachine {
    type Output = ();

    fn load_bytecode_section<'bc>(
        &mut self,
        section: BytecodeSectionView<'bc, TextContext>,
    ) -> eyre::Result<()> {
        self.info = section.decode_header()?;
        load_bytecode_program(&mut self.program, section)
    }
}

#[vihaco::composite]
#[derive(Debug, Default)]
#[allow(dead_code)]
struct TextHeaderMachine {
    info: TestHeader,

    program: TextProgram,

    #[device(0x01)]
    device: TextLoadedDevice,
}

impl LoadSSTSection<TextContext> for TextHeaderMachine {
    type Output = ();

    fn load_sst_section<'bc>(
        &mut self,
        section: SSTSectionView<'bc, TextContext>,
    ) -> eyre::Result<()> {
        let parsed = ParsedModule::<TextInst, TestHeader>::parse_section(section.clone())?;
        self.info = parsed.header;
        load_parsed_text_program(&mut self.program, parsed, section.context_handle())?;
        Ok(())
    }
}

#[test]
fn parses_surface_instruction_without_runtime_bytecode_traits() {
    let file = text_file(
        &[],
        ".section main\n\
         .text\n\
         @main:\n\
         \x20   surface.nop\n",
    );

    let parsed =
        ParsedModule::<SurfaceOnlyInst, NoHeader>::parse_section(file.section("main").unwrap())
            .unwrap();

    assert_eq!(parsed.code, vec![SurfaceOnlyInst::Nop]);
    assert_eq!(parsed.symbols[0].name, "main");
}

#[test]
fn binary_sections_load_independently_by_name() {
    let controller = encode(&[TestInst::Nop]);
    let readout = encode(&[TestInst::Load(ConstantId(0))]);
    let file: BytecodeFile<TextContext> = BytecodeFile::from_bytes(binary_file(
        b"controller\nreadout\n",
        &[
            (CONTROLLER_NAME, b"", &controller),
            (READOUT_NAME, b"", &readout),
        ],
    ))
    .unwrap();

    let mut controller = LoadedDevice::default();
    controller
        .load_bytecode_section(file.section("controller").unwrap())
        .unwrap();
    let mut readout = LoadedDevice::default();
    readout
        .load_bytecode_section(file.section("readout").unwrap())
        .unwrap();

    assert_eq!(controller.program.module.code, vec![TestInst::Nop]);
    assert_eq!(
        readout.program.module.code,
        vec![TestInst::Load(ConstantId(0))]
    );
    for program in [&controller.program, &readout.program] {
        assert!(
            program
                .context
                .as_ref()
                .unwrap()
                .ptr_eq(&file.context_handle())
        );
    }
    assert_eq!(
        readout.program.get_constant(ConstantId(0)).unwrap(),
        &Value::I64(9)
    );
}

#[test]
fn text_sections_load_independently_by_name() {
    let file = text_file(
        &["controller", "readout"],
        ".section controller\n\
         .text\n\
         @main:\n\
         \x20   test.nop\n\
         \n\
         .section readout\n\
         .text\n\
         @main:\n\
         \x20   test.alt\n",
    );

    let mut controller = TextLoadedDevice::default();
    controller
        .load_sst_section(file.section("controller").unwrap())
        .unwrap();
    let mut readout = TextLoadedDevice::default();
    readout
        .load_sst_section(file.section("readout").unwrap())
        .unwrap();

    assert_eq!(controller.program.module.code, vec![TextInst::Nop]);
    assert_eq!(readout.program.module.code, vec![TextInst::Alt]);
    assert!(
        readout
            .program
            .context
            .as_ref()
            .unwrap()
            .ptr_eq(&file.context_handle())
    );
    assert_eq!(
        file.sections()
            .map(|section| section.name())
            .collect::<Vec<_>>(),
        ["controller", "readout"]
    );
}

#[test]
fn binary_composite_loads_its_section_header() {
    let mut header = Vec::new();
    TestHeader { cores: 8 }.write_bytes(&mut header).unwrap();
    let code = encode(&[TestInst::Nop]);
    let file: BytecodeFile<TextContext> = BytecodeFile::from_bytes(binary_file(
        b"controller\n",
        &[(CONTROLLER_NAME, &header, &code)],
    ))
    .unwrap();

    let mut machine = HeaderMachine::default();
    machine
        .load_bytecode_section(file.section("controller").unwrap())
        .unwrap();

    assert_eq!(machine.info, TestHeader { cores: 8 });
    assert_eq!(machine.program.module.code, vec![TestInst::Nop]);
}

#[test]
fn text_composite_loads_its_section_header() {
    let file = text_file(
        &[],
        ".section controller\n\
         .header\n\
         \x20   8\n\
         .text\n\
         @main:\n\
         \x20   test.nop\n",
    );

    let mut machine = TextHeaderMachine::default();
    machine
        .load_sst_section(file.section("controller").unwrap())
        .unwrap();

    assert_eq!(machine.info, TestHeader { cores: 8 });
    assert_eq!(machine.program.module.code, vec![TextInst::Nop]);
}

fn encode(instructions: &[TestInst]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for inst in instructions {
        inst.write_bytes(&mut bytes).unwrap();
    }
    bytes
}

/// Encode a binary file whose sections are `(name_string, header, code)`.
fn binary_file(context: &[u8], sections: &[(u32, &[u8], &[u8])]) -> Vec<u8> {
    const FILE_HEADER_LEN: usize = 4 + 2 + 2 + 8;
    const SECTION_COUNT_LEN: usize = 4;
    const SECTION_TABLE_ENTRY_LEN: usize = 4 + 8 * 4;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.write_u16::<LittleEndian>(VERSION).unwrap();
    bytes.write_u16::<LittleEndian>(FLAGS).unwrap();
    bytes
        .write_u64::<LittleEndian>(context.len() as u64)
        .unwrap();
    bytes.extend_from_slice(context);
    bytes
        .write_u32::<LittleEndian>(sections.len() as u32)
        .unwrap();

    let mut offset = FILE_HEADER_LEN
        + context.len()
        + SECTION_COUNT_LEN
        + sections.len() * SECTION_TABLE_ENTRY_LEN;
    for (name, header, code) in sections {
        bytes.write_u32::<LittleEndian>(*name).unwrap();
        for value in [offset, header.len(), offset + header.len(), code.len()] {
            bytes.write_u64::<LittleEndian>(value as u64).unwrap();
        }
        offset += header.len() + code.len();
    }
    for (_, header, code) in sections {
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(code);
    }
    bytes
}

fn text_file(section_names: &[&str], sections: &str) -> SSTFile<TextContext> {
    let context: String = section_names
        .iter()
        .map(|name| format!("    {name}\n"))
        .collect();
    SSTFile::<TextContext>::from_text(&format!(
        "sst v{SST_VERSION}\n\n.global\n{context}\n{sections}"
    ))
    .unwrap()
}
