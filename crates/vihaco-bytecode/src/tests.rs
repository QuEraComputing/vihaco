// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::*;
use crate::NoContext;
use crate::file::{BytecodeFile, SSTFile};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Read;
use vihaco_abi::Instruction;
use vihaco_abi::traits::{FromBytes, FromText, WriteBytes};

#[derive(Debug, Clone, PartialEq, Instruction)]
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

impl SSTGlobalContext for WrappedContext {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(Self {
            inner: <TextContext as SSTGlobalContext>::from_text(text)?.into(),
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

impl SSTGlobalContext for TextContext {
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

// ---------------------------------------------------------------------------
// Binary container
// ---------------------------------------------------------------------------

/// One section to encode: name-string index, header bytes, code bytes.
type BinarySection<'a> = (u32, &'a [u8], &'a [u8]);

/// Encode a well-formed binary file with a flat, contiguous section table.
fn binary_file(context: &[u8], sections: &[BinarySection<'_>]) -> Vec<u8> {
    let data_start = crate::container::bytecode::FILE_HEADER_LEN
        + context.len()
        + crate::container::bytecode::SECTION_COUNT_LEN
        + sections.len() * crate::container::bytecode::SECTION_TABLE_ENTRY_LEN;
    let mut offset = data_start;
    let entries = sections
        .iter()
        .map(|(name, header, code)| {
            let header_offset = offset;
            let code_offset = header_offset + header.len();
            offset = code_offset + code.len();
            (*name, header_offset, header.len(), code_offset, code.len())
        })
        .collect::<Vec<_>>();
    let mut bytes = raw_binary_file(context, &entries);
    for (_, header, code) in sections {
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(code);
    }
    bytes
}

/// Encode a file header, context, and section table with explicit entries.
///
/// Entries are `(name, header_offset, header_len, code_offset, code_len)`.
fn raw_binary_file(context: &[u8], entries: &[(u32, usize, usize, usize, usize)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.write_u16::<LittleEndian>(VERSION).unwrap();
    bytes.write_u16::<LittleEndian>(FLAGS).unwrap();
    bytes
        .write_u64::<LittleEndian>(context.len() as u64)
        .unwrap();
    bytes.extend_from_slice(context);
    bytes
        .write_u32::<LittleEndian>(entries.len() as u32)
        .unwrap();
    for &(name, header_offset, header_len, code_offset, code_len) in entries {
        bytes.write_u32::<LittleEndian>(name).unwrap();
        for value in [header_offset, header_len, code_offset, code_len] {
            bytes.write_u64::<LittleEndian>(value as u64).unwrap();
        }
    }
    bytes
}

fn encode(instructions: &[TestInst]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for inst in instructions {
        inst.write_bytes(&mut bytes).unwrap();
    }
    bytes
}

fn context_with_strings(strings: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes
        .write_u32::<LittleEndian>(strings.len() as u32)
        .unwrap();
    for string in strings {
        bytes
            .write_u32::<LittleEndian>(string.len() as u32)
            .unwrap();
        bytes.extend_from_slice(string.as_bytes());
    }
    bytes
}

fn read_string(bytes: &mut &[u8]) -> eyre::Result<String> {
    let len = bytes.read_u32::<LittleEndian>()? as usize;
    if bytes.len() < len {
        return Err(eyre::eyre!("string length {len} exceeds remaining bytes"));
    }
    let (value, rest) = bytes.split_at(len);
    *bytes = rest;
    Ok(std::str::from_utf8(value)?.to_string())
}

fn parse_binary(bytes: Vec<u8>) -> eyre::Result<BytecodeFile<SectionNameContext>> {
    BytecodeFile::from_bytes(bytes)
}

#[test]
fn parses_binary_flat_section_table() {
    let cpu_code = encode(&[TestInst::Load(ConstantId(0))]);
    let alu_code = encode(&[TestInst::Nop]);
    let file = binary_file(
        &context_with_strings(&["main", "cpu", "alu"]),
        &[(1, b"cpu header", &cpu_code), (2, b"alu header", &alu_code)],
    );

    let parsed = parse_binary(file).unwrap();
    assert_eq!(parsed.context().section_names, ["main", "cpu", "alu"]);
    assert_eq!(
        parsed.sections().map(|s| s.name()).collect::<Vec<_>>(),
        ["cpu", "alu"]
    );

    let cpu = parsed.section("cpu").unwrap();
    assert_eq!(cpu.header_bytes(), b"cpu header");
    assert_eq!(
        cpu.decode_instructions::<TestInst>().unwrap(),
        [TestInst::Load(ConstantId(0))]
    );
    let alu = parsed.section("alu").unwrap();
    assert_eq!(alu.header_bytes(), b"alu header");
    assert_eq!(
        alu.decode_instructions::<TestInst>().unwrap(),
        [TestInst::Nop]
    );
    assert!(parsed.section("main").is_none());
}

#[test]
fn parses_binary_file_with_no_sections() {
    let parsed = parse_binary(binary_file(&context_with_strings(&[]), &[])).unwrap();
    assert_eq!(parsed.sections().len(), 0);
}

#[test]
fn parses_binary_file_with_custom_context_representation() {
    let code = encode(&[TestInst::Nop]);
    let parsed: BytecodeFile<WrappedContext> = BytecodeFile::from_bytes(binary_file(
        &context_with_strings(&["cpu"]),
        &[(0, b"", &code)],
    ))
    .unwrap();

    assert_eq!(parsed.context().inner.section_names, ["cpu"]);
    assert_eq!(parsed.section("cpu").unwrap().name(), "cpu");
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

    let context = context_with_strings(&["main"]);
    let header = 99u32.to_le_bytes();
    let parsed = parse_binary(binary_file(&context, &[(0, &header, b"")])).unwrap();
    let main = parsed.section("main").unwrap();
    assert_eq!(main.decode_header::<Header>().unwrap(), Header(99));

    let long_header = [&header[..], b"x"].concat();
    let parsed = parse_binary(binary_file(&context, &[(0, &long_header, b"")])).unwrap();
    let err = parsed
        .section("main")
        .unwrap()
        .decode_header::<Header>()
        .unwrap_err();
    assert!(err.to_string().contains("1 trailing bytes"), "{err}");
}

#[test]
fn rejects_bad_binary_magic() {
    let mut bytes = binary_file(&context_with_strings(&[]), &[]);
    bytes[0] = b'X';
    let err = parse_binary(bytes).unwrap_err();
    assert!(err.to_string().contains("invalid bytecode magic"));
}

#[test]
fn rejects_bad_binary_version() {
    let mut bytes = binary_file(&context_with_strings(&[]), &[]);
    bytes[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
    let err = parse_binary(bytes).unwrap_err();
    assert!(err.to_string().contains("unsupported bytecode version"));
}

#[test]
fn rejects_binary_missing_section_name_string() {
    let err = parse_binary(binary_file(&context_with_strings(&[]), &[(0, b"", b"")])).unwrap_err();
    assert!(err.to_string().contains("missing section name string"));
}

#[test]
fn rejects_binary_duplicate_section_names() {
    let err = parse_binary(binary_file(
        &context_with_strings(&["cpu"]),
        &[(0, b"", b""), (0, b"", b"")],
    ))
    .unwrap_err();
    assert!(err.to_string().contains("duplicate section `cpu`"), "{err}");
}

#[test]
fn rejects_binary_truncated_section_table() {
    let mut bytes = binary_file(&context_with_strings(&["cpu"]), &[(0, b"", b"")]);
    bytes.truncate(bytes.len() - 1);
    let err = parse_binary(bytes).unwrap_err();
    assert!(err.to_string().contains("complete section table"), "{err}");
}

#[test]
fn rejects_binary_section_data_past_end_of_file() {
    let context = context_with_strings(&["cpu"]);
    let bytes = raw_binary_file(&context, &[(0, 0, 0, 999, 4)]);
    let err = parse_binary(bytes).unwrap_err();
    assert!(
        err.to_string().contains("extends past end of bytecode"),
        "{err}"
    );
}

#[test]
fn rejects_binary_section_data_inside_section_table() {
    let context = context_with_strings(&["cpu"]);
    let bytes = raw_binary_file(&context, &[(0, 0, 0, 0, 4)]);
    let err = parse_binary(bytes).unwrap_err();
    assert!(
        err.to_string().contains("before the section table ends"),
        "{err}"
    );
}

#[test]
fn rejects_binary_overlapping_sections() {
    let context = context_with_strings(&["cpu", "gpu"]);
    let data_start = binary_file(&context, &[(0, b"", b""), (1, b"", b"")]).len();
    let mut bytes = raw_binary_file(
        &context,
        &[(0, 0, 0, data_start, 4), (1, data_start + 2, 2, 0, 0)],
    );
    bytes.extend_from_slice(&[0; 4]);
    let err = parse_binary(bytes).unwrap_err();
    assert!(
        err.to_string()
            .contains("section `gpu` header overlaps section `cpu` code"),
        "{err}"
    );
}

#[test]
fn accepts_empty_ranges_that_point_inside_other_sections() {
    // An empty range occupies no bytes, so it cannot overlap anything.
    let context = context_with_strings(&["cpu", "gpu"]);
    let data_start = binary_file(&context, &[(0, b"", b""), (1, b"", b"")]).len();
    let mut bytes = raw_binary_file(
        &context,
        &[
            (0, 0, 0, data_start, 4),
            (1, data_start + 2, 0, data_start + 1, 0),
        ],
    );
    bytes.extend_from_slice(&[0; 4]);
    let file = parse_binary(bytes).unwrap();
    let gpu = file.section("gpu").unwrap();
    assert!(gpu.header_bytes().is_empty() && gpu.bytecode().is_empty());
    assert_eq!(file.section("cpu").unwrap().bytecode().len(), 4);
}

#[test]
fn rejects_binary_instruction_stream_with_non_multiple_width() {
    let err = decode_instruction_stream::<TestInst>(&[0, 1, 2]).unwrap_err();
    assert!(err.to_string().contains("not a multiple"));
}

// ---------------------------------------------------------------------------
// SST text container
// ---------------------------------------------------------------------------

fn sst(body: &str) -> String {
    format!("sst v{SST_VERSION}\n{body}")
}

fn parse_sst(body: &str) -> eyre::Result<SSTFile<TextContext>> {
    SSTFile::from_text(&sst(body))
}

#[test]
fn parses_sst_context_and_flat_sections() {
    let parsed = parse_sst(
        "\n.global\n    cpu\n    alu\n\n\
         .section cpu  // the controller\n\
         .header\n    cpu header\n\
         .text\n    cpu text\n\n\
         .section alu\n\
         .text\n    alu text\n",
    )
    .unwrap();

    assert_eq!(parsed.context().section_names, ["cpu", "alu"]);
    assert_eq!(
        parsed.sections().map(|s| s.name()).collect::<Vec<_>>(),
        ["cpu", "alu"]
    );

    let cpu = parsed.section("cpu").unwrap();
    assert_eq!(cpu.header_text(), "    cpu header\n");
    assert_eq!(cpu.text(), "    cpu text\n\n");

    let alu = parsed.section("alu").unwrap();
    assert_eq!(alu.header_text(), "");
    assert_eq!(alu.text(), "    alu text\n");
}

#[test]
fn sst_blocks_may_appear_in_either_order() {
    let parsed = parse_sst(".section main\n.text\nt\n.header\nh\n").unwrap();
    let main = parsed.section("main").unwrap();
    assert_eq!(main.header_text(), "h\n");
    assert_eq!(main.text(), "t\n");
}

#[test]
fn sst_block_content_is_passed_through_verbatim() {
    // Lines that merely resemble directives belong to the enclosing block.
    let body = "  .texture 1\n  sst value\n  // comment\n  .section_like x\n";
    let parsed = parse_sst(&format!(".section main\n.text\n{body}")).unwrap();
    assert_eq!(parsed.section("main").unwrap().text(), body);
}

#[test]
fn parses_sst_with_crlf_line_endings_and_indented_directives() {
    let parsed = parse_sst("\t.section main\r\n  .text\r\nbody\r\n").unwrap();
    assert_eq!(parsed.section("main").unwrap().text(), "body\r\n");
}

#[test]
fn parses_sst_file_with_no_sections() {
    let parsed = SSTFile::<NoContext>::from_text("sst v2\n").unwrap();
    assert_eq!(parsed.sections().len(), 0);
}

#[test]
fn parses_sst_empty_context_when_global_is_omitted() {
    let parsed = parse_sst(".section main\n").unwrap();
    assert!(parsed.context().section_names.is_empty());

    let parsed = SSTFile::<NoContext>::from_text(&sst(".global\n\n.section main\n")).unwrap();
    assert_eq!(parsed.context(), &NoContext);
}

#[test]
fn rejects_no_context_file_with_non_empty_global() {
    let err = SSTFile::<NoContext>::from_text(&sst(".global\ncpu\n.section main\n")).unwrap_err();
    assert!(
        err.to_string()
            .contains("NoContext accepts only an empty global section")
    );
}

#[test]
fn rejects_sst_bad_or_missing_version() {
    let err = SSTFile::<NoContext>::from_text("sst v1\n.section main\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("unsupported sst version 1 (expected 2)"),
        "{err}"
    );

    let err = SSTFile::<NoContext>::from_text(".section main\n").unwrap_err();
    assert!(err.to_string().contains("expected `sst v2`"), "{err}");

    let err = SSTFile::<NoContext>::from_text("").unwrap_err();
    assert!(err.to_string().contains("expected `sst v2`"), "{err}");
}

#[test]
fn rejects_sst_duplicate_sections() {
    let err = parse_sst(".section cpu\n.section cpu\n").unwrap_err();
    assert!(
        err.to_string().contains("line 3: duplicate section `cpu`"),
        "{err}"
    );
}

#[test]
fn rejects_sst_duplicate_blocks() {
    let err = parse_sst(".section cpu\n.header\n.header\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("section `cpu` declares duplicate `.header`"),
        "{err}"
    );

    let err = parse_sst(".section cpu\n.text\n.text\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("section `cpu` declares duplicate `.text`"),
        "{err}"
    );
}

#[test]
fn rejects_sst_blocks_outside_a_section() {
    let err = parse_sst(".text\nbody\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("`.text` must follow a `.section <name>` directive"),
        "{err}"
    );
}

#[test]
fn rejects_sst_content_outside_a_block() {
    let err = parse_sst("stray\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("line 2: unexpected content; expected `.global` or `.section <name>`"),
        "{err}"
    );

    let err = parse_sst(".section cpu\nstray\n").unwrap_err();
    assert!(
        err.to_string().contains(
            "line 3: unexpected content; expected `.header`, `.text`, or `.section <name>`"
        ),
        "{err}"
    );
}

#[test]
fn rejects_sst_misplaced_or_duplicate_global() {
    let err = parse_sst(".section cpu\n.global\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("`.global` must come before any `.section`"),
        "{err}"
    );

    let err = parse_sst(".global\n.global\n").unwrap_err();
    assert!(
        err.to_string().contains("duplicate `.global` block"),
        "{err}"
    );
}

#[test]
fn rejects_sst_malformed_directives() {
    let err = parse_sst(".section\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("`.section` expects a section name"),
        "{err}"
    );

    let err = parse_sst(".section a b\n").unwrap_err();
    assert!(err.to_string().contains("expects a single name"), "{err}");

    let err = parse_sst(".section cpu\n.text cpu\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("`.text` takes no arguments, found `cpu`"),
        "{err}"
    );
}

#[test]
fn scan_reports_every_framing_error_with_locations() {
    let text = "sst v2\nstray\nmore\n.section a b\n.section cpu\n.header\n.header\n.section cpu\n.text x\n";
    let layout = crate::scan_sst(text);
    let errors: Vec<_> = layout
        .errors
        .iter()
        .map(|error| {
            (
                error.line,
                &text[error.span.clone()],
                error.message.as_str(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        [
            (
                2,
                "stray",
                "unexpected content; expected `.global` or `.section <name>`"
            ),
            (4, "a b", "`.section` expects a single name, found `a b`"),
            (7, ".header", "section `cpu` declares duplicate `.header`"),
            (8, "cpu", "duplicate section `cpu`"),
            (9, "x", "`.text` takes no arguments, found `x`"),
        ]
    );
    let (related, note) = layout.errors[3].related.clone().unwrap();
    assert_eq!(
        (&text[related], note.as_str()),
        ("cpu", "first defined here")
    );
    assert_eq!(layout.sections.len(), 3);
    let header = layout.sections[1].header.clone().unwrap();
    assert_eq!(&text[header.directive], ".header");
    assert_eq!(&text[header.body], "");
    // The fail-fast parser reports the first classification error.
    let err = SSTFile::<NoContext>::from_text(text).unwrap_err();
    assert_eq!(
        err.to_string(),
        "line 4: `.section` expects a single name, found `a b`"
    );
}

#[test]
fn section_views_expose_file_spans() {
    let text = "sst v2\n.section main\n.header\nh\n.text\nt\n";
    let file = SSTFile::<NoContext>::from_text(text).unwrap();
    let section = file.section("main").unwrap();
    assert_eq!(&section.file_text()[section.header_span()], "h\n");
    assert_eq!(&section.file_text()[section.text_span()], "t\n");
}
