// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};
use chumsky::{error::Simple, extra, prelude::*};

use crate::{
    binary::{BytecodeContext, ConstantId},
    module::{FunctionInfo, LabelInfo, Parameter, Signature, SourceSymbolInfo},
    traits::{FromBytes, FromText},
};

#[path = "value.rs"]
pub mod value;

pub use value::{Type, Value};

type ContextParseExtra<'src> = extra::Err<Simple<'src, char>>;

pub trait ProgramGlobals {
    type Type;
    type Value;

    fn get_function(&self, index: usize) -> eyre::Result<FunctionInfo<Self::Type>>;
    fn get_string(&self, index: usize) -> eyre::Result<&String>;
    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value>;
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProgramContext<V = Value, Ty = Type> {
    pub constants: Vec<V>,
    pub strings: Vec<String>,
    pub functions: Vec<FunctionInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub main_function: Option<u32>,
    pub file: u32,
    pub source_symbols: Vec<SourceSymbolInfo>,
}

impl<V, Ty> ProgramContext<V, Ty>
where
    V: FromBytes,
    Ty: FromBytes,
{
    pub fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let context = Self::read_from(&mut cursor)?;
        if cursor.position() as usize != bytes.len() {
            return Err(eyre::eyre!(
                "program context has {} trailing bytes",
                bytes.len() - cursor.position() as usize
            ));
        }
        Ok(context)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> eyre::Result<Self> {
        let constants = read_vec(reader, "constant", V::from_bytes)?;
        let strings = read_vec(reader, "string", read_string)?;
        let functions = read_vec(reader, "function", read_function_info)?;
        let labels = read_vec(reader, "label", read_label_info)?;
        let main_function = read_optional_u32(reader)?;
        let file = reader.read_u32::<LittleEndian>()?;
        let source_symbols = read_vec(reader, "source symbol", read_source_symbol_info)?;

        Ok(Self {
            constants,
            strings,
            functions,
            labels,
            main_function,
            file,
            source_symbols,
        })
    }
}

impl<V, Ty> ProgramContext<V, Ty>
where
    V: FromText,
    Ty: FromText,
{
    pub fn from_text(text: &str) -> eyre::Result<Self> {
        let normalized = normalize_context_text(text);
        context_text_parser::<V, Ty>()
            .parse(normalized.as_str())
            .into_result()
            .map_err(format_context_parse_errors)
    }
}

impl<V, Ty> BytecodeContext for ProgramContext<V, Ty>
where
    V: FromBytes + FromText,
    Ty: FromBytes + FromText,
{
    fn from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        ProgramContext::from_bytes(bytes)
    }

    fn section_name(&self, index: u32) -> Option<&str> {
        self.strings.get(index as usize).map(String::as_str)
    }

    fn from_text(text: &str) -> eyre::Result<Self> {
        ProgramContext::from_text(text)
    }
}

impl<V, Ty> ProgramGlobals for ProgramContext<V, Ty>
where
    Ty: Clone,
{
    type Type = Ty;
    type Value = V;

    fn get_function(&self, index: usize) -> eyre::Result<FunctionInfo<Self::Type>> {
        self.functions.get(index).cloned().ok_or_else(|| {
            eyre::eyre!(format!(
                "function index out of bounds: {} (max {})",
                index,
                self.functions.len()
            ))
        })
    }

    fn get_string(&self, index: usize) -> eyre::Result<&String> {
        self.strings.get(index).ok_or_else(|| {
            eyre::eyre!(format!(
                "string index out of bounds: {} (max {})",
                index,
                self.strings.len()
            ))
        })
    }

    fn get_constant(&self, id: ConstantId) -> eyre::Result<&Self::Value> {
        self.constants.get(id.0 as usize).ok_or_else(|| {
            eyre::eyre!(format!(
                "constant index out of bounds: {} (max {})",
                id.0,
                self.constants.len()
            ))
        })
    }
}

fn read_vec<R, T, F>(reader: &mut R, label: &str, mut read_item: F) -> eyre::Result<Vec<T>>
where
    R: Read,
    F: FnMut(&mut R) -> eyre::Result<T>,
{
    let count = reader.read_u32::<LittleEndian>()? as usize;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(
            read_item(reader)
                .map_err(|err| eyre::eyre!("failed to read {label} table entry {index}: {err}"))?,
        );
    }
    Ok(values)
}

fn read_string<R: Read>(reader: &mut R) -> eyre::Result<String> {
    let len = reader.read_u32::<LittleEndian>()? as usize;
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn read_function_info<R, Ty>(reader: &mut R) -> eyre::Result<FunctionInfo<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let name = reader.read_u32::<LittleEndian>()?;
    let signature = read_signature(reader)?;
    let local_count = reader.read_u32::<LittleEndian>()?;
    let start_address = reader.read_u32::<LittleEndian>()?;
    let end_address = reader.read_u32::<LittleEndian>()?;
    let file = reader.read_u32::<LittleEndian>()?;

    Ok(FunctionInfo {
        name,
        signature,
        local_count,
        start_address,
        end_address,
        file,
    })
}

fn read_signature<R, Ty>(reader: &mut R) -> eyre::Result<Signature<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let params = read_vec(reader, "parameter", read_parameter)?;
    let ret = read_vec(reader, "return type", Ty::from_bytes)?;
    Ok(Signature { params, ret })
}

fn read_parameter<R, Ty>(reader: &mut R) -> eyre::Result<Parameter<Ty>>
where
    R: Read,
    Ty: FromBytes,
{
    let name = reader.read_u32::<LittleEndian>()?;
    let ty = Ty::from_bytes(reader)?;
    Ok(Parameter { name, ty })
}

fn read_label_info<R: Read>(reader: &mut R) -> eyre::Result<LabelInfo> {
    let address = reader.read_u32::<LittleEndian>()?;
    let name = reader.read_u32::<LittleEndian>()?;
    Ok(LabelInfo { address, name })
}

fn read_source_symbol_info<R: Read>(reader: &mut R) -> eyre::Result<SourceSymbolInfo> {
    let index = reader.read_u32::<LittleEndian>()?;
    let name = read_string(reader)?;
    Ok(SourceSymbolInfo { index, name })
}

fn read_optional_u32<R: Read>(reader: &mut R) -> eyre::Result<Option<u32>> {
    match reader.read_u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.read_u32::<LittleEndian>()?)),
        other => Err(eyre::eyre!(
            "invalid optional u32 discriminant {} in program context",
            other
        )),
    }
}

fn context_text_parser<'src, V, Ty>()
-> impl Parser<'src, &'src str, ProgramContext<V, Ty>, ContextParseExtra<'src>>
where
    V: FromText + 'src,
    Ty: FromText + 'src,
{
    let constants = context_line_entry::<V>(".strings")
        .repeated()
        .collect::<Vec<_>>();
    let strings = string_line().repeated().collect::<Vec<_>>();
    let functions = function_line::<Ty>().repeated().collect::<Vec<_>>();
    let labels = label_line().repeated().collect::<Vec<_>>();
    let source_symbols = source_symbol_line().repeated().collect::<Vec<_>>();

    heading(".constants")
        .ignore_then(constants)
        .then_ignore(heading(".strings"))
        .then(strings)
        .then_ignore(heading(".functions"))
        .then(functions)
        .then_ignore(heading(".labels"))
        .then(labels)
        .then(main_function_line())
        .then(file_line())
        .then_ignore(heading(".source-symbols"))
        .then(source_symbols)
        .then_ignore(end())
        .map(
            |(
                (((((constants, strings), functions), labels), main_function), file),
                source_symbols,
            )| {
                ProgramContext {
                    constants,
                    strings,
                    functions,
                    labels,
                    main_function,
                    file,
                    source_symbols,
                }
            },
        )
}

fn heading<'src>(
    name: &'static str,
) -> impl Parser<'src, &'src str, (), ContextParseExtra<'src>> + Clone {
    just(name).then_ignore(eol()).ignored()
}

fn eol<'src>() -> impl Parser<'src, &'src str, (), ContextParseExtra<'src>> + Clone {
    just('\r').or_not().then_ignore(just('\n')).ignored()
}

fn inline_ws<'src>() -> impl Parser<'src, &'src str, (), ContextParseExtra<'src>> + Clone {
    one_of(" \t").repeated().ignored()
}

fn required_inline_ws<'src>() -> impl Parser<'src, &'src str, (), ContextParseExtra<'src>> + Clone {
    one_of(" \t").repeated().at_least(1).ignored()
}

fn u32_text<'src>() -> impl Parser<'src, &'src str, u32, ContextParseExtra<'src>> + Clone {
    text::int(10)
        .try_map(|text: &str, span| text.parse::<u32>().map_err(|_| Simple::new(None, span)))
}

fn context_line_entry<'src, T>(
    next_marker: &'static str,
) -> impl Parser<'src, &'src str, T, ContextParseExtra<'src>>
where
    T: FromText + 'src,
{
    just(next_marker)
        .not()
        .ignore_then(
            any()
                .filter(|ch: &char| !matches!(*ch, '\r' | '\n'))
                .repeated()
                .at_least(1)
                .to_slice(),
        )
        .then_ignore(eol())
        .try_map(|text: &str, span| {
            parse_text_entry(text.trim()).map_err(|_| Simple::new(None, span))
        })
}

fn string_line<'src>() -> impl Parser<'src, &'src str, String, ContextParseExtra<'src>> + Clone {
    string_literal().then_ignore(inline_ws()).then_ignore(eol())
}

fn function_line<'src, Ty>()
-> impl Parser<'src, &'src str, FunctionInfo<Ty>, ContextParseExtra<'src>>
where
    Ty: FromText + 'src,
{
    just("fn")
        .ignore_then(required_inline_ws())
        .ignore_then(u32_text())
        .then_ignore(required_inline_ws())
        .then(signature_text::<Ty>())
        .then_ignore(required_inline_ws())
        .then(u32_text())
        .then_ignore(required_inline_ws())
        .then(u32_text())
        .then_ignore(required_inline_ws())
        .then(u32_text())
        .then_ignore(required_inline_ws())
        .then(u32_text())
        .then_ignore(inline_ws())
        .then_ignore(eol())
        .map(
            |(((((name, signature), local_count), start_address), end_address), file)| {
                FunctionInfo {
                    name,
                    signature,
                    local_count,
                    start_address,
                    end_address,
                    file,
                }
            },
        )
}

fn signature_text<'src, Ty>() -> impl Parser<'src, &'src str, Signature<Ty>, ContextParseExtra<'src>>
where
    Ty: FromText + 'src,
{
    just('(')
        .ignore_then(inline_ws())
        .ignore_then(
            parameter_text::<Ty>()
                .separated_by(comma_separator())
                .collect::<Vec<_>>(),
        )
        .then_ignore(inline_ws())
        .then_ignore(just(')'))
        .then_ignore(inline_ws())
        .then_ignore(just("->"))
        .then_ignore(inline_ws())
        .then(return_types_text::<Ty>())
        .map(|(params, ret)| Signature { params, ret })
}

fn parameter_text<'src, Ty>() -> impl Parser<'src, &'src str, Parameter<Ty>, ContextParseExtra<'src>>
where
    Ty: FromText + 'src,
{
    u32_text()
        .then_ignore(inline_ws())
        .then_ignore(just(':'))
        .then_ignore(inline_ws())
        .then(type_text::<Ty>())
        .map(|(name, ty)| Parameter { name, ty })
}

fn return_types_text<'src, Ty>() -> impl Parser<'src, &'src str, Vec<Ty>, ContextParseExtra<'src>>
where
    Ty: FromText + 'src,
{
    let parenthesized = just('(')
        .ignore_then(inline_ws())
        .ignore_then(
            type_text::<Ty>()
                .separated_by(comma_separator())
                .collect::<Vec<_>>(),
        )
        .then_ignore(inline_ws())
        .then_ignore(just(')'));

    parenthesized.or(type_text::<Ty>().map(|ty| vec![ty]))
}

fn type_text<'src, Ty>() -> impl Parser<'src, &'src str, Ty, ContextParseExtra<'src>>
where
    Ty: FromText + 'src,
{
    any()
        .filter(|ch: &char| !ch.is_whitespace() && !matches!(*ch, ',' | ')'))
        .repeated()
        .at_least(1)
        .to_slice()
        .try_map(|text: &str, span| parse_text_entry(text).map_err(|_| Simple::new(None, span)))
}

fn comma_separator<'src>() -> impl Parser<'src, &'src str, (), ContextParseExtra<'src>> + Clone {
    inline_ws()
        .ignore_then(just(','))
        .then_ignore(inline_ws())
        .ignored()
}

fn label_line<'src>() -> impl Parser<'src, &'src str, LabelInfo, ContextParseExtra<'src>> + Clone {
    u32_text()
        .then_ignore(required_inline_ws())
        .then(u32_text())
        .then_ignore(inline_ws())
        .then_ignore(eol())
        .map(|(address, name)| LabelInfo { address, name })
}

fn main_function_line<'src>()
-> impl Parser<'src, &'src str, Option<u32>, ContextParseExtra<'src>> + Clone {
    just(".main")
        .ignore_then(required_inline_ws())
        .ignore_then(just("none").to(None).or(u32_text().map(Some)))
        .then_ignore(inline_ws())
        .then_ignore(eol())
}

fn file_line<'src>() -> impl Parser<'src, &'src str, u32, ContextParseExtra<'src>> + Clone {
    just(".file")
        .ignore_then(required_inline_ws())
        .ignore_then(u32_text())
        .then_ignore(inline_ws())
        .then_ignore(eol())
}

fn source_symbol_line<'src>()
-> impl Parser<'src, &'src str, SourceSymbolInfo, ContextParseExtra<'src>> + Clone {
    u32_text()
        .then_ignore(required_inline_ws())
        .then(string_literal())
        .then_ignore(inline_ws())
        .then_ignore(eol())
        .map(|(index, name)| SourceSymbolInfo { index, name })
}

fn string_literal<'src>() -> impl Parser<'src, &'src str, String, ContextParseExtra<'src>> + Clone {
    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('0').to('\0'),
    )));
    let char_or_escape = choice((
        escape,
        any().and_is(just('"').not()).and_is(just('\\').not()),
    ));
    just('"')
        .ignore_then(char_or_escape.repeated().collect::<String>())
        .then_ignore(just('"'))
}

fn parse_text_entry<T: FromText>(text: &str) -> eyre::Result<T> {
    let mut cursor = Cursor::new(text.as_bytes());
    T::from_text(&mut cursor)
}

fn normalize_context_text(text: &str) -> String {
    let mut normalized = String::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        normalized.push_str(line);
        normalized.push('\n');
    }
    normalized
}

fn format_context_parse_errors(errors: Vec<Simple<'_, char>>) -> eyre::Report {
    let error = errors
        .into_iter()
        .next()
        .map(|error| format!("{error:?}"))
        .unwrap_or_else(|| "unknown parse error".to_string());
    eyre::eyre!("failed to parse text program context: {error}")
}
