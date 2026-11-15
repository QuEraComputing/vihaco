// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Standard program-header schema and section-local metadata validation.
//!
//! Every table is supplied by the header, even when empty. Array order defines
//! table indices; names and files are explicit string-table indices. Addresses
//! count decoded instructions, with an exclusive function end. Loading never
//! interns strings, invents signatures, or infers an entrypoint.
//!
//! Constants use their type's ordinary `Parse` implementation. Standard `Value`
//! constants use typed SST literals such as `u64 42` or `string 0`; `f32_bits`
//! and `f64_bits` followed by unsigned integers preserve all IEEE-754 encodings.
//! Custom extra records use the same delimited metadata derive as the header.
//!
//! Validation ([`ProgramMetadata::validate`], and the lookups
//! [`MetadataLoweringContext`](crate::loader::MetadataLoweringContext)
//! prepares) reports every problem with the same message shape: the metadata
//! path, then what is wrong with expected and found values, for example
//! `metadata.functions[0].name: string 9 is outside the string table (length
//! 2)`.

use std::collections::HashMap;
use std::fmt;

use crate::module::{
    FunctionInfo, LabelInfo, LocalModule, NamedTypeInfo, NoInfo, SourceSymbolInfo, StructInfo,
    StructTypeRef,
};
use crate::source_type::{ResolveType, TypeLookup};
use vihaco_abi::{Type, Value};

/// All non-code fields of a `LocalModule`, supplied explicitly by a section header.
/// The ordinary metadata derive owns object parsing, including nested records.
#[derive(Debug, Clone, PartialEq, vihaco_parser_derive::Parse)]
#[syntax_class(metadata, head = "metadata", open = "{", close = "}")]
pub struct ProgramMetadata<V = Value, Ty = Type, Info = NoInfo> {
    pub functions: Vec<FunctionInfo<Ty>>,
    pub structs: Vec<StructInfo<Ty>>,
    /// The named-type table: `type N` refers to entry `N`.
    pub types: Vec<NamedTypeInfo<Ty>>,
    pub labels: Vec<LabelInfo>,
    pub constants: Vec<V>,
    pub strings: Vec<String>,
    pub main_function: Option<u32>,
    pub source_symbols: Vec<SourceSymbolInfo>,
    pub extra: Info,
}

/// A metadata table that indices refer to, for messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Table {
    Strings,
    Functions,
    Structs,
    Types,
}

impl Table {
    /// What one entry is called: `string 9`.
    fn entry(self) -> &'static str {
        match self {
            Table::Strings => "string",
            Table::Functions => "function",
            Table::Structs => "struct",
            Table::Types => "named type",
        }
    }

    /// What the table is called: `the string table`.
    fn name(self) -> &'static str {
        match self {
            Table::Strings => "string table",
            Table::Functions => "function table",
            Table::Structs => "struct table",
            Table::Types => "named-type table",
        }
    }
}

/// Check that `index` is an entry of `table` (of length `len`), reporting
/// `{path}: string 9 is outside the string table (length 2)` otherwise.
pub(crate) fn check_index(
    path: &dyn fmt::Display,
    index: u32,
    table: Table,
    len: usize,
) -> eyre::Result<()> {
    eyre::ensure!(
        (index as usize) < len,
        "{path}: {} {index} is outside the {} (length {len})",
        table.entry(),
        table.name()
    );
    Ok(())
}

/// The string at `index`, checked as [`check_index`] does.
fn string_at<'a>(
    strings: &'a [String],
    index: u32,
    path: &dyn fmt::Display,
) -> eyre::Result<&'a str> {
    check_index(path, index, Table::Strings, strings.len())?;
    Ok(&strings[index as usize])
}

/// Record a declaration's name, reporting a duplicate with both paths.
fn declare<'a>(
    seen: &mut HashMap<&'a str, String>,
    name: &'a str,
    what: &str,
    path: String,
    within: &str,
) -> eyre::Result<()> {
    if let Some(first) = seen.get(name) {
        eyre::bail!("{path}: duplicate {what} `{name}`{within} (first declared at {first})");
    }
    seen.insert(name, path);
    Ok(())
}

/// The name tables of a section, built once with every name checked: each
/// name is a string-table entry, and names are unique per table (and per
/// struct, for fields).
///
/// [`ProgramMetadata::validate`] builds these to check names, and
/// [`MetadataLoweringContext`](crate::loader::MetadataLoweringContext) keeps
/// them for lookups, so both report the same messages.
#[derive(Debug)]
pub(crate) struct NameTables {
    /// Function name to (function index, start address).
    pub(crate) functions: HashMap<String, (u32, u32)>,
    /// Struct name to struct-table index.
    pub(crate) struct_names: HashMap<String, u32>,
    /// Per struct-table entry: its name and fields.
    pub(crate) structs: Vec<StructFields>,
    /// Named-type name to named-type-table index.
    pub(crate) type_names: HashMap<String, u32>,
}

#[derive(Debug)]
pub(crate) struct StructFields {
    pub(crate) name: String,
    pub(crate) count: u32,
    /// Field name to field slot.
    pub(crate) slots: HashMap<String, u32>,
}

impl<V, Ty, Info> ProgramMetadata<V, Ty, Info> {
    /// Transform consumer-specific extra metadata while preserving every table.
    pub fn map_extra<NewInfo>(
        self,
        map: impl FnOnce(Info) -> NewInfo,
    ) -> ProgramMetadata<V, Ty, NewInfo> {
        ProgramMetadata {
            functions: self.functions,
            structs: self.structs,
            types: self.types,
            labels: self.labels,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: map(self.extra),
        }
    }

    /// Build the section's name tables, checking every name.
    pub(crate) fn name_tables(&self) -> eyre::Result<NameTables> {
        let strings = &self.strings;
        let mut functions = HashMap::new();
        let mut seen = HashMap::new();
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}].name");
            let name = string_at(strings, function.name, &path)?;
            declare(&mut seen, name, "function", path, "")?;
            functions.insert(
                name.to_owned(),
                (u32::try_from(index)?, function.start_address),
            );
        }
        let mut struct_names = HashMap::new();
        let mut structs = Vec::with_capacity(self.structs.len());
        let mut seen = HashMap::new();
        for (index, info) in self.structs.iter().enumerate() {
            let path = format!("metadata.structs[{index}]");
            let name = string_at(strings, info.name, &format_args!("{path}.name"))?;
            declare(&mut seen, name, "struct", format!("{path}.name"), "")?;
            let mut slots = HashMap::new();
            let mut fields = HashMap::new();
            for (slot, field) in info.fields.iter().enumerate() {
                let field_path = format!("{path}.fields[{slot}].name");
                let field_name = string_at(strings, field.name, &field_path)?;
                let within = format!(" in struct `{name}`");
                declare(&mut fields, field_name, "field", field_path, &within)?;
                slots.insert(field_name.to_owned(), u32::try_from(slot)?);
            }
            struct_names.insert(name.to_owned(), u32::try_from(index)?);
            structs.push(StructFields {
                name: name.to_owned(),
                count: u32::try_from(info.fields.len())?,
                slots,
            });
        }
        let mut type_names = HashMap::new();
        let mut seen = HashMap::new();
        for (index, info) in self.types.iter().enumerate() {
            let path = format!("metadata.types[{index}].name");
            let name = string_at(strings, info.name, &path)?;
            declare(&mut seen, name, "named type", path, "")?;
            type_names.insert(name.to_owned(), u32::try_from(index)?);
        }
        Ok(NameTables {
            functions,
            struct_names,
            structs,
            type_names,
        })
    }

    /// Resolve every type in the tables to `T`: for example, a header parsed
    /// with [`SourceType`](crate::module::SourceType), whose struct and named
    /// types may be written by name (`locals: [struct @Point]`), resolves to
    /// [`Type`] against its own struct and named-type tables.
    ///
    /// # Errors
    /// An undefined name, reported with its metadata path:
    /// `metadata.functions[0].locals[1]: undefined struct `@Pointt``.
    pub fn resolve_types<T>(self) -> eyre::Result<ProgramMetadata<V, T, Info>>
    where
        Ty: ResolveType<T>,
    {
        let lookup = TypeLookup::new(&self.structs, &self.types, &self.strings);
        let resolve = |ty: &Ty, path: &dyn Fn() -> String| {
            ty.resolve_type(&lookup)
                .map_err(|error| eyre::eyre!("{}: {error}", path()))
        };
        let mut functions = Vec::with_capacity(self.functions.len());
        for (index, info) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}]");
            let mut params = Vec::with_capacity(info.signature.params.len());
            for (param, parameter) in info.signature.params.iter().enumerate() {
                params.push(crate::module::Parameter {
                    name: parameter.name,
                    ty: resolve(&parameter.ty, &|| {
                        format!("{path}.signature.params[{param}].ty")
                    })?,
                });
            }
            let ret = (info.signature.ret.iter().enumerate())
                .map(|(ret, ty)| resolve(ty, &|| format!("{path}.signature.ret[{ret}]")))
                .collect::<eyre::Result<_>>()?;
            let locals = (info.locals.iter().enumerate())
                .map(|(local, ty)| resolve(ty, &|| format!("{path}.locals[{local}]")))
                .collect::<eyre::Result<_>>()?;
            functions.push(FunctionInfo {
                name: info.name,
                signature: crate::module::Signature { params, ret },
                locals,
                start_address: info.start_address,
                end_address: info.end_address,
                file: info.file,
            });
        }
        let mut structs = Vec::with_capacity(self.structs.len());
        for (index, info) in self.structs.iter().enumerate() {
            let fields = (info.fields.iter().enumerate())
                .map(|(field, entry)| {
                    Ok(crate::module::FieldInfo {
                        name: entry.name,
                        ty: resolve(&entry.ty, &|| {
                            format!("metadata.structs[{index}].fields[{field}].ty")
                        })?,
                    })
                })
                .collect::<eyre::Result<_>>()?;
            structs.push(StructInfo {
                name: info.name,
                fields,
            });
        }
        let types = (self.types.iter().enumerate())
            .map(|(index, info)| {
                Ok(NamedTypeInfo {
                    name: info.name,
                    repr: resolve(&info.repr, &|| format!("metadata.types[{index}].repr"))?,
                })
            })
            .collect::<eyre::Result<_>>()?;
        drop(lookup);
        Ok(ProgramMetadata {
            functions,
            structs,
            types,
            labels: self.labels,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: self.extra,
        })
    }
}

impl<V, Ty: PartialEq + Clone + fmt::Display, Info> ProgramMetadata<V, Ty, Info> {
    /// Validate references and instruction ranges without altering supplied metadata.
    ///
    /// Names are string-table entries, unique per table (fields per struct).
    /// Each function declares at most [`MAX_LOCALS`](crate::module::MAX_LOCALS)
    /// locals, at least as many as its parameters, and its leading locals are
    /// its parameter types (see [`FunctionInfo::check_frame`]).
    pub fn validate(&self, instruction_count: usize) -> eyre::Result<()> {
        eyre::ensure!(
            u32::try_from(instruction_count).is_ok(),
            "metadata: {instruction_count} instructions exceed the u32 address space"
        );
        if let Some(main) = self.main_function {
            check_index(
                &"metadata.main_function",
                main,
                Table::Functions,
                self.functions.len(),
            )?;
        }
        self.name_tables()?;
        let names = self.names();
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}]");
            check_index(
                &format_args!("{path}.file"),
                function.file,
                Table::Strings,
                self.strings.len(),
            )?;
            eyre::ensure!(
                function.start_address <= function.end_address
                    && function.end_address as usize <= instruction_count,
                "{path}: instruction range {}..{} does not fit in the {instruction_count} \
                 instructions of the section",
                function.start_address,
                function.end_address
            );
            for (param, parameter) in function.signature.params.iter().enumerate() {
                check_index(
                    &format_args!("{path}.signature.params[{param}].name"),
                    parameter.name,
                    Table::Strings,
                    self.strings.len(),
                )?;
            }
            if let Err(error) = function.check_frame() {
                let at = match &error {
                    crate::module::FrameError::LocalDisagreesWithParameter { index, .. } => {
                        format!("{path}.locals[{index}]")
                    }
                    _ => format!("{path}.locals"),
                };
                let function = index as u32;
                let message = error.display_with(
                    move |param| names.parameter_name(function, param),
                    Ty::to_string,
                );
                eyre::bail!("{at}: {message}");
            }
        }
        for (index, label) in self.labels.iter().enumerate() {
            check_index(
                &format_args!("metadata.labels[{index}].name"),
                label.name,
                Table::Strings,
                self.strings.len(),
            )?;
            eyre::ensure!(
                label.address as usize <= instruction_count,
                "metadata.labels[{index}].address: address {} is past the end of the \
                 {instruction_count} instructions of the section",
                label.address
            );
        }
        let mut symbols = HashMap::new();
        for (position, symbol) in self.source_symbols.iter().enumerate() {
            if let Some(first) = symbols.insert(symbol.index, position) {
                eyre::bail!(
                    "metadata.source_symbols[{position}].index: duplicate index {} (first \
                     declared at metadata.source_symbols[{first}])",
                    symbol.index
                );
            }
        }
        Ok(())
    }
}

impl<V, Ty: StructTypeRef, Info> ProgramMetadata<V, Ty, Info> {
    /// Check that every struct type (`struct N`) and named type (`type N`) in
    /// signatures, locals and struct fields names an entry of its table,
    /// including references nested inside array element types, and that every
    /// named type is represented by a scalar word type. Structs may refer to
    /// themselves or each other.
    pub fn validate_struct_types(&self) -> eyre::Result<()> {
        let check = |ty: &Ty, path: &dyn Fn() -> String| {
            let mut result = Ok(());
            ty.visit_struct_refs(&mut |index| {
                if result.is_ok() {
                    result = check_index(&path(), index, Table::Structs, self.structs.len());
                }
            });
            ty.visit_named_refs(&mut |index| {
                if result.is_ok() {
                    result = check_index(&path(), index, Table::Types, self.types.len());
                }
            });
            result
        };
        for (index, info) in self.types.iter().enumerate() {
            eyre::ensure!(
                info.repr.is_named_type_repr(),
                "metadata.types[{index}].repr: a named type must be represented by \
                 i32, i64, u32, u64, f32, f64 or bool"
            );
        }
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("metadata.functions[{index}].signature");
            for (param, parameter) in function.signature.params.iter().enumerate() {
                check(&parameter.ty, &|| format!("{path}.params[{param}].ty"))?;
            }
            for (ret, ty) in function.signature.ret.iter().enumerate() {
                check(ty, &|| format!("{path}.ret[{ret}]"))?;
            }
            for (local, ty) in function.locals.iter().enumerate() {
                check(ty, &|| {
                    format!("metadata.functions[{index}].locals[{local}]")
                })?;
            }
        }
        for (index, info) in self.structs.iter().enumerate() {
            for (field, info) in info.fields.iter().enumerate() {
                check(&info.ty, &|| {
                    format!("metadata.structs[{index}].fields[{field}].ty")
                })?;
            }
        }
        Ok(())
    }
}

impl<Ty: PartialEq + Clone + fmt::Display + StructTypeRef, Info> ProgramMetadata<Value, Ty, Info> {
    /// Assemble the standard value model, checking the supplied tables once.
    /// Custom value models can assemble `LocalModule` directly after `validate`
    /// and `validate_struct_types`.
    pub fn into_module<I>(self, code: Vec<I>) -> eyre::Result<LocalModule<I, Value, Ty, Info>> {
        self.validate(code.len())?;
        self.validate_struct_types()?;
        for (index, constant) in self.constants.iter().enumerate() {
            match constant {
                Value::String(id) => check_index(
                    &format_args!("metadata.constants[{index}]"),
                    *id,
                    Table::Strings,
                    self.strings.len(),
                )?,
                Value::FunctionRef(id) => check_index(
                    &format_args!("metadata.constants[{index}]"),
                    *id,
                    Table::Functions,
                    self.functions.len(),
                )?,
                _ => {}
            }
        }
        Ok(LocalModule {
            code,
            functions: self.functions,
            structs: self.structs,
            types: self.types,
            labels: self.labels,
            constants: self.constants,
            strings: self.strings,
            main_function: self.main_function,
            source_symbols: self.source_symbols,
            extra: self.extra,
        })
    }
}

/// Parse a metadata header on its own: `metadata { ... }`, with surrounding
/// whitespace and comments.
///
/// On failure, headers written for an older format get a specific message
/// (see [`explain_metadata`]); other errors report the parser's.
impl<V, Ty, Info> vihaco_abi::traits::FromText for ProgramMetadata<V, Ty, Info>
where
    for<'src> Self: vihaco_parser::Parse<'src>,
{
    fn from_text(text: &str) -> eyre::Result<Self> {
        use chumsky::Parser as _;
        <Self as vihaco_parser::Parse<'_>>::parser()
            .padded_by(vihaco_parser::trivia())
            .then_ignore(chumsky::prelude::end())
            .parse(text)
            .into_result()
            .map_err(|errors| match explain_metadata(text) {
                Some(explanation) => eyre::eyre!("invalid metadata: {explanation}"),
                None => eyre::eyre!("invalid metadata: {errors:?}"),
            })
    }
}

/// The tables every `metadata { ... }` header lists, in order, and what an
/// empty one looks like.
const REQUIRED: [(&str, &str); 9] = [
    ("functions", "[]"),
    ("structs", "[]"),
    ("types", "[]"),
    ("labels", "[]"),
    ("constants", "[]"),
    ("strings", "[]"),
    ("main_function", "null"),
    ("source_symbols", "[]"),
    ("extra", "{}"),
];

/// Explain why `text`, a header containing `metadata { ... }` that failed to
/// parse, does not match the current format, when the cause is a known
/// change: a function's `local_count` (replaced by `locals: [..]`, one type
/// per local) or a missing table (every table is required, even when empty).
///
/// Returns `None` when neither applies, or when `text` has no metadata
/// object; the parser's own error is then the best description.
///
/// ```
/// # use vihaco_module::module::explain_metadata;
/// let old = "metadata { functions: [{ name: 0, local_count: 1 }] }";
/// assert_eq!(
///     explain_metadata(old).unwrap(),
///     "metadata.functions[0]: `local_count` was replaced by `locals: [..]`, which lists \
///      the type of every local, parameters first"
/// );
/// ```
pub fn explain_metadata(text: &str) -> Option<String> {
    let keys = scan_metadata(text)?;
    if let Some(function) = keys.local_count {
        let at = match function {
            Some(index) => format!("metadata.functions[{index}]"),
            None => "metadata".to_owned(),
        };
        return Some(format!(
            "{at}: `local_count` was replaced by `locals: [..]`, which lists the type of every \
             local, parameters first"
        ));
    }
    let missing: Vec<_> = REQUIRED
        .iter()
        .filter(|(key, _)| !keys.top_level.iter().any(|found| found == key))
        .collect();
    match missing.as_slice() {
        [] => None,
        [(key, empty)] => Some(format!(
            "metadata: missing required table `{key}` (write `{key}: {empty}` if it is empty)"
        )),
        many => Some(format!(
            "metadata: missing required tables {} (write {} if they are empty)",
            many.iter()
                .map(|(key, _)| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", "),
            many.iter()
                .map(|(key, empty)| format!("`{key}: {empty}`"))
                .collect::<Vec<_>>()
                .join(", "),
        )),
    }
}

/// What a lightweight scan of a metadata object found.
struct MetadataKeys {
    /// Keys of the metadata object itself.
    top_level: Vec<String>,
    /// Where `local_count` appeared: `Some(Some(i))` inside function `i`.
    local_count: Option<Option<usize>>,
}

/// Scan the first `metadata { ... }` object in `text` for its keys, skipping
/// strings and `//` comments. Tolerates malformed input: the scan only looks
/// at keys (identifiers followed by `:`) and nesting.
fn scan_metadata(text: &str) -> Option<MetadataKeys> {
    struct Open {
        brace: bool,
        /// The key whose value this container is.
        key: Option<String>,
        /// How many objects this array has opened.
        objects: usize,
    }
    let chars: Vec<char> = text.chars().collect();
    let mut stack: Vec<Open> = Vec::new();
    let mut seen_head = false;
    let mut found_object = false;
    let mut pending: Option<String> = None;
    let mut keys = MetadataKeys {
        top_level: Vec::new(),
        local_count: None,
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    i += if chars[i] == '\\' { 2 } else { 1 };
                }
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '{' | '[' => {
                if stack.is_empty() {
                    if !(c == '{' && seen_head) || found_object {
                        i += 1;
                        continue;
                    }
                    found_object = true;
                }
                let brace = c == '{';
                if brace && let Some(parent) = stack.last_mut().filter(|open| !open.brace) {
                    parent.objects += 1;
                }
                stack.push(Open {
                    brace,
                    key: pending.take(),
                    objects: 0,
                });
            }
            '}' | ']' => {
                stack.pop();
                pending = None;
                if found_object && stack.is_empty() {
                    break;
                }
            }
            ',' => pending = None,
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let mut j = i;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if stack.is_empty() {
                    seen_head |= word == "metadata";
                } else if chars.get(j) == Some(&':') {
                    if stack.len() == 1 {
                        keys.top_level.push(word.clone());
                    }
                    if word == "local_count" && keys.local_count.is_none() {
                        let function = (stack.len() == 3
                            && stack[1].key.as_deref() == Some("functions"))
                        .then(|| stack[1].objects.saturating_sub(1));
                        keys.local_count = Some(function);
                    }
                    pending = Some(word);
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    found_object.then_some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_abi::traits::FromText;

    const CURRENT: &str = "metadata {
        functions: [{ name: 0, signature: { params: [], ret: [] }, locals: [],
                      start_address: 0, end_address: 1, file: 0 }],
        structs: [], types: [], labels: [], constants: [], strings: [\"main\"],
        main_function: 0, source_symbols: [], extra: {},
    }";

    #[test]
    fn current_headers_parse() {
        let metadata = ProgramMetadata::<Value, Type>::from_text(CURRENT).unwrap();
        assert_eq!(metadata.functions.len(), 1);
        assert_eq!(explain_metadata(CURRENT), None);
    }

    #[test]
    fn old_local_counts_name_their_replacement() {
        let old = CURRENT.replace("locals: []", "local_count: 1");
        let err = ProgramMetadata::<Value, Type>::from_text(&old).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid metadata: metadata.functions[0]: `local_count` was replaced by \
             `locals: [..]`, which lists the type of every local, parameters first"
        );
        let second = CURRENT.replace("file: 0 }]", "file: 0 }, { name: 0, local_count: 0 }]");
        assert!(
            explain_metadata(&second)
                .unwrap()
                .starts_with("metadata.functions[1]:")
        );
    }

    #[test]
    fn missing_tables_are_named_with_their_empty_form() {
        let err = ProgramMetadata::<Value, Type>::from_text(&CURRENT.replace("types: [], ", ""))
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid metadata: metadata: missing required table `types` (write `types: []` \
             if it is empty)"
        );
        let both = CURRENT.replace("structs: [], types: [], ", "");
        assert_eq!(
            explain_metadata(&both).unwrap(),
            "metadata: missing required tables `structs`, `types` (write `structs: []`, \
             `types: []` if they are empty)"
        );
        // Keys inside strings, comments, and nested objects do not count.
        let hidden = CURRENT.replace(
            "structs: [], types: [], ",
            "// types: []\n structs: [{ name: 0, fields: [], types: [] }], ",
        );
        assert_eq!(
            explain_metadata(&hidden).unwrap(),
            "metadata: missing required table `types` (write `types: []` if it is empty)"
        );
        assert_eq!(explain_metadata("not a header"), None);
    }

    #[test]
    fn other_parse_errors_keep_the_parser_message() {
        let err = ProgramMetadata::<Value, Type>::from_text(&CURRENT.replace("file: 0", "file: x"))
            .unwrap_err();
        assert!(err.to_string().starts_with("invalid metadata: ["), "{err}");
    }
}
