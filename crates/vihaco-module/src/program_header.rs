// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The standard SST header: flat, assembler-style directives that name things
//! by symbol, assembled into [`ProgramMetadata`].
//!
//! ```text
//! .header
//!     .file "loadable.sst"
//!     .func @helper() -> u64, locals 0
//!     .func @main(x: u64) -> (u64, bool), locals 2
//!     .entry @main
//!     .const u64 42
//!     .const str "hello"
//!     .const fn @helper
//!     .source_symbol 12 "user"
//! ```
//!
//! No table index is written by hand. [`ProgramHeader`] collects directives
//! in header order and derives every table:
//!
//! - **functions**: one per `.func`, in `.func` order. A `.func` belongs to
//!   the file named by the closest `.file` before it, or to the empty file
//!   `""` if there is none.
//! - **entry**: `.entry @name` selects the function declared by
//!   `.func @name`. It is optional and may appear at most once.
//! - **constants**: one per `.const`, in `.const` order. `.const str "text"`
//!   is a string constant and `.const fn @name` a function reference; other
//!   values use the value type's own syntax, such as `u64 42`.
//! - **source symbols**: one per `.source_symbol <index> "<name>"`; indices
//!   are unique.
//! - **strings**: each distinct string once, at its first occurrence, in this
//!   order: function names in `.func` order, then parameter names (by
//!   function, then by position), then file names in `.func` order, then
//!   `.const str` literals in `.const` order. Lowering then appends the
//!   string literals of `.text` in address order
//!   (`MetadataLoweringContext` interns them), so the same file always
//!   produces the same table.
//!
//! Application directives compose with these in one header: an application
//! declaration enum has a variant holding a [`ProgramDirective`] (usually
//! with `#[pattern = "$0"]`) and implements [`StandardDirective`]; a loader
//! pushes the standard directives into a [`ProgramHeader`] and handles the
//! others itself.

use std::collections::HashMap;
use std::fmt;

use chumsky::error::Simple;
use chumsky::extra;
use chumsky::prelude::*;
use vihaco_abi::{Type, Value};
use vihaco_parser::{Parse, Print, QuotedString, print_quoted, trivia};

use crate::module::{FunctionDecl, Parameter, ProgramMetadata, Signature, SourceSymbolInfo};

type E<'src> = extra::Err<Simple<'src, char>>;

/// Characters allowed in a symbol or parameter name.
fn name_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            ':' | '@' | ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '"' | '\'' | '`'
        )
}

fn name<'src>() -> impl Parser<'src, &'src str, String, E<'src>> + Clone {
    any()
        .filter(|c: &char| name_char(*c))
        .repeated()
        .at_least(1)
        .collect::<String>()
}

fn print_name(text: &str, out: &mut dyn fmt::Write) -> fmt::Result {
    // A name that starts like an arrow or a number is still a valid name;
    // only the characters matter.
    if text.is_empty() || !text.chars().all(name_char) {
        return Err(fmt::Error);
    }
    out.write_str(text)
}

/// `( item, item, ... )` with trivia (and so line breaks) inside.
fn parenthesized<'src, T>(
    item: impl Parser<'src, &'src str, T, E<'src>>,
) -> impl Parser<'src, &'src str, Vec<T>, E<'src>> {
    item.separated_by(just(',').then(trivia()))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('(').then(trivia()), just(')'))
}

/// A value that can live in the constant pool of a standard header.
///
/// `.const str "text"` and `.const fn @name` are assembled through
/// [`string`](Self::string) and [`function`](Self::function); a value model
/// without string or function constants returns `None` and such directives
/// are rejected. [`string_index`](Self::string_index) and
/// [`function_index`](Self::function_index) report the table references of
/// a value, so that a hand-written index such as `.const str 3` is rejected
/// and tools can explain constants.
pub trait ConstantValue: Sized {
    /// The constant for `strings[index]`.
    fn string(index: u32) -> Option<Self> {
        let _ = index;
        None
    }

    /// The constant for `functions[index]`.
    fn function(index: u32) -> Option<Self> {
        let _ = index;
        None
    }

    /// The `strings` index this constant refers to, if any.
    fn string_index(&self) -> Option<u32> {
        None
    }

    /// The `functions` index this constant refers to, if any.
    fn function_index(&self) -> Option<u32> {
        None
    }
}

impl ConstantValue for Value {
    fn string(index: u32) -> Option<Self> {
        Some(Value::String(index))
    }

    fn function(index: u32) -> Option<Self> {
        Some(Value::FunctionRef(index))
    }

    fn string_index(&self) -> Option<u32> {
        match self {
            Value::String(index) => Some(*index),
            _ => None,
        }
    }

    fn function_index(&self) -> Option<u32> {
        match self {
            Value::FunctionRef(index) => Some(*index),
            _ => None,
        }
    }
}

/// `.func @name(param: type, ...) -> ret, locals N`.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDirective<Ty = Type> {
    /// The function's name, without `@`. It must match an `@name:` symbol in
    /// `.text`.
    pub name: String,
    /// Parameter names and types, in order.
    pub params: Vec<(String, Ty)>,
    /// Return types: none (`-> ...` omitted), one (`-> u64`), or several
    /// (`-> (u64, bool)`).
    pub ret: Vec<Ty>,
    /// Reserved local slots, including parameters.
    pub locals: u32,
}

/// The value of a `.const` directive.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstantDirective<V = Value> {
    /// `.const str "text"`: a string constant, interned in the string table.
    Str(String),
    /// `.const fn @name`: a reference to the function declared by
    /// `.func @name`.
    Function(String),
    /// `.const <value>` in the value type's own syntax, such as `u64 42`.
    Value(V),
}

/// One standard header directive.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgramDirective<V = Value, Ty = Type> {
    /// `.file "path"`: the file of the `.func` directives that follow.
    File(String),
    /// `.func @name(...) -> ..., locals N`: a function declaration.
    Function(FunctionDirective<Ty>),
    /// `.entry @name`: the entry function.
    Entry(String),
    /// `.const <value>`: the next constant-pool entry.
    Const(ConstantDirective<V>),
    /// `.source_symbol <index> "<name>"`: a source-level symbol.
    SourceSymbol(SourceSymbolInfo),
}

impl<'src, V, Ty> Parse<'src> for ProgramDirective<V, Ty>
where
    V: Parse<'src> + 'src,
    Ty: Parse<'src> + 'src,
{
    fn parser() -> impl Parser<'src, &'src str, Self, E<'src>> {
        let space = one_of(" \t").repeated().at_least(1);
        let gap = one_of(" \t").repeated();
        let symbol = just('@').ignore_then(name());
        let keyword = |word: &'static str| just(word).then(space);

        let file = keyword(".file")
            .ignore_then(QuotedString::parser())
            .map(|path| ProgramDirective::File(path.into_inner()));

        let ty = || Ty::parser().padded_by(trivia());
        let param = name().padded_by(trivia()).then_ignore(just(':')).then(ty());
        let ret = gap
            .ignore_then(just("->"))
            .ignore_then(gap)
            .ignore_then(choice((
                parenthesized(ty()),
                Ty::parser().map(|ty| vec![ty]),
            )))
            .or_not()
            .map(Option::unwrap_or_default);
        let function = keyword(".func")
            .ignore_then(symbol.clone())
            .then(parenthesized(param))
            .then(ret)
            .then_ignore(gap)
            .then_ignore(just(','))
            .then_ignore(gap)
            .then_ignore(keyword("locals"))
            .then(u32::parser())
            .map(|(((name, params), ret), locals)| {
                ProgramDirective::Function(FunctionDirective {
                    name,
                    params,
                    ret,
                    locals,
                })
            });

        let entry = keyword(".entry")
            .ignore_then(symbol.clone())
            .map(ProgramDirective::Entry);

        let constant = keyword(".const")
            .ignore_then(choice((
                keyword("str")
                    .ignore_then(QuotedString::parser())
                    .map(|text| ConstantDirective::Str(text.into_inner())),
                keyword("fn")
                    .ignore_then(symbol)
                    .map(ConstantDirective::Function),
                V::parser().map(ConstantDirective::Value),
            )))
            .map(ProgramDirective::Const);

        let source_symbol = keyword(".source_symbol")
            .ignore_then(u32::parser())
            .then_ignore(space)
            .then(QuotedString::parser())
            .map(|(index, name)| {
                ProgramDirective::SourceSymbol(SourceSymbolInfo {
                    index,
                    name: name.into_inner(),
                })
            });

        choice((file, function, entry, constant, source_symbol))
    }
}

impl<V: Print, Ty: Print> Print for ProgramDirective<V, Ty> {
    fn print(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            ProgramDirective::File(path) => {
                out.write_str(".file ")?;
                print_quoted(path, out)
            }
            ProgramDirective::Function(function) => {
                out.write_str(".func @")?;
                print_name(&function.name, out)?;
                out.write_char('(')?;
                for (index, (name, ty)) in function.params.iter().enumerate() {
                    if index > 0 {
                        out.write_str(", ")?;
                    }
                    print_name(name, out)?;
                    out.write_str(": ")?;
                    ty.print(out)?;
                }
                out.write_char(')')?;
                match function.ret.as_slice() {
                    [] => {}
                    [ty] => {
                        out.write_str(" -> ")?;
                        ty.print(out)?;
                    }
                    types => {
                        out.write_str(" -> (")?;
                        for (index, ty) in types.iter().enumerate() {
                            if index > 0 {
                                out.write_str(", ")?;
                            }
                            ty.print(out)?;
                        }
                        out.write_char(')')?;
                    }
                }
                write!(out, ", locals {}", function.locals)
            }
            ProgramDirective::Entry(name) => {
                out.write_str(".entry @")?;
                print_name(name, out)
            }
            ProgramDirective::Const(constant) => {
                out.write_str(".const ")?;
                match constant {
                    ConstantDirective::Str(text) => {
                        out.write_str("str ")?;
                        print_quoted(text, out)
                    }
                    ConstantDirective::Function(name) => {
                        out.write_str("fn @")?;
                        print_name(name, out)
                    }
                    ConstantDirective::Value(value) => {
                        let mut text = String::new();
                        value.print(&mut text)?;
                        // A value spelled like the string or function forms
                        // would be read back as them.
                        if text.starts_with("str \"") || text.starts_with("fn @") {
                            return Err(fmt::Error);
                        }
                        out.write_str(&text)
                    }
                }
            }
            ProgramDirective::SourceSymbol(symbol) => {
                write!(out, ".source_symbol {} ", symbol.index)?;
                print_quoted(&symbol.name, out)
            }
        }
    }
}

/// A header declaration that may be a standard directive.
///
/// Implemented by [`ProgramDirective`] itself. An application declaration
/// enum implements it by returning its standard variant, so that checkers,
/// formatters, and loaders can find the standard directives among the
/// application's own:
///
/// ```
/// use vihaco_module::module::{ProgramDirective, StandardDirective};
///
/// enum Declaration {
///     Standard(ProgramDirective),
///     Description(String),
/// }
///
/// impl StandardDirective for Declaration {
///     type Value = vihaco_abi::Value;
///     type Type = vihaco_abi::Type;
///
///     fn as_standard(&self) -> Option<&ProgramDirective> {
///         match self {
///             Self::Standard(directive) => Some(directive),
///             Self::Description(_) => None,
///         }
///     }
/// }
/// ```
pub trait StandardDirective {
    type Value;
    type Type;

    fn as_standard(&self) -> Option<&ProgramDirective<Self::Value, Self::Type>>;
}

impl<V, Ty> StandardDirective for ProgramDirective<V, Ty> {
    type Value = V;
    type Type = Ty;

    fn as_standard(&self) -> Option<&ProgramDirective<V, Ty>> {
        Some(self)
    }
}

/// Which part of a directive a [`HeaderError`] is about, so tools can point
/// at it precisely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The whole directive.
    Directive,
    /// The `@name` in the directive.
    Symbol,
    /// The `locals N` part of a `.func`.
    Locals,
    /// Everything after the directive keyword.
    Argument,
}

/// A problem found while assembling a [`ProgramHeader`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderError {
    /// The position of the offending directive among the pushed directives.
    pub directive: usize,
    pub focus: Focus,
    pub message: String,
    /// A short annotation for the focused text.
    pub label: String,
    /// An earlier directive the problem relates to, such as the first
    /// declaration of a duplicated function.
    pub related: Option<(usize, Focus, String)>,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HeaderError {}

/// Standard header directives in header order, assembled into
/// [`ProgramMetadata`]. See the [module documentation](self) for the
/// derivation rules.
///
/// ```
/// use vihaco_module::module::{NoInfo, ProgramHeader};
/// use vihaco_parser::Parse;
/// use chumsky::Parser;
///
/// let header: ProgramHeader = [".file \"a.sst\"", ".func @main() -> u64, locals 0", ".entry @main"]
///     .into_iter()
///     .map(|line| vihaco_module::module::ProgramDirective::parser().parse(line).into_result().unwrap())
///     .collect();
/// let metadata = header.finish(NoInfo).unwrap();
/// assert_eq!(metadata.strings, ["main", "a.sst"]);
/// assert_eq!(metadata.main_function, Some(0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramHeader<V = Value, Ty = Type> {
    directives: Vec<ProgramDirective<V, Ty>>,
}

impl<V, Ty> Default for ProgramHeader<V, Ty> {
    fn default() -> Self {
        Self {
            directives: Vec::new(),
        }
    }
}

impl<V, Ty> FromIterator<ProgramDirective<V, Ty>> for ProgramHeader<V, Ty> {
    fn from_iter<T: IntoIterator<Item = ProgramDirective<V, Ty>>>(iter: T) -> Self {
        Self {
            directives: iter.into_iter().collect(),
        }
    }
}

impl<V, Ty> Extend<ProgramDirective<V, Ty>> for ProgramHeader<V, Ty> {
    fn extend<T: IntoIterator<Item = ProgramDirective<V, Ty>>>(&mut self, iter: T) {
        self.directives.extend(iter);
    }
}

/// Strings in first-occurrence order.
#[derive(Default)]
struct Interner {
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl Interner {
    fn intern(&mut self, text: &str) -> u32 {
        if let Some(&index) = self.index.get(text) {
            return index;
        }
        let index = self.strings.len() as u32;
        self.strings.push(text.to_owned());
        self.index.insert(text.to_owned(), index);
        index
    }
}

impl<V, Ty> ProgramHeader<V, Ty> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append the next directive.
    pub fn push(&mut self, directive: ProgramDirective<V, Ty>) {
        self.directives.push(directive);
    }

    /// The directives pushed so far, in order.
    pub fn directives(&self) -> &[ProgramDirective<V, Ty>] {
        &self.directives
    }

    /// Assemble the tables, reporting every problem.
    ///
    /// Problems are duplicate `.func` names, `locals` smaller than the
    /// parameter count, more than one `.entry`, an `.entry` or `.const fn`
    /// naming no `.func`, a constant written as a table index (`.const str 3`
    /// or `.const fn 0`), string or function constants the value model cannot
    /// represent, and duplicate `.source_symbol` indices. Whether each
    /// function has an `@name:` symbol is checked when the metadata is bound
    /// to `.text` ([`ProgramMetadata::validate`]).
    pub fn assemble<Info>(
        self,
        extra: Info,
    ) -> Result<ProgramMetadata<V, Ty, Info>, Vec<HeaderError>>
    where
        V: ConstantValue,
    {
        let mut errors = Vec::new();
        let error = |directive, focus, message: String, label: &str| HeaderError {
            directive,
            focus,
            message,
            label: label.to_owned(),
            related: None,
        };

        let mut file = None::<String>;
        let mut functions = Vec::new();
        let mut entries = Vec::new();
        let mut constants = Vec::new();
        let mut source_symbols = Vec::new();
        for (position, directive) in self.directives.into_iter().enumerate() {
            match directive {
                ProgramDirective::File(path) => file = Some(path),
                ProgramDirective::Function(function) => {
                    functions.push((position, function, file.clone().unwrap_or_default()))
                }
                ProgramDirective::Entry(name) => entries.push((position, name)),
                ProgramDirective::Const(constant) => constants.push((position, constant)),
                ProgramDirective::SourceSymbol(symbol) => source_symbols.push((position, symbol)),
            }
        }

        let mut by_name = HashMap::new();
        let mut duplicates = Vec::new();
        for (index, (position, function, _)) in functions.iter().enumerate() {
            if let Some(&(first, _)) = by_name.get(function.name.as_str()) {
                duplicates.push(HeaderError {
                    directive: *position,
                    focus: Focus::Symbol,
                    message: format!("function `@{}` is declared more than once", function.name),
                    label: "declared again here".to_owned(),
                    related: Some((first, Focus::Symbol, "first declared here".to_owned())),
                });
            } else {
                by_name.insert(function.name.as_str(), (*position, index as u32));
            }
            if (function.locals as usize) < function.params.len() {
                errors.push(error(
                    *position,
                    Focus::Locals,
                    format!(
                        "`@{}` reserves {} local slots but has {} parameters",
                        function.name,
                        function.locals,
                        function.params.len()
                    ),
                    &format!("must be at least {}", function.params.len()),
                ));
            }
        }

        let mut main_function = None;
        for (count, (position, name)) in entries.iter().enumerate() {
            if count > 0 {
                errors.push(HeaderError {
                    directive: *position,
                    focus: Focus::Directive,
                    message: "the entry function is selected more than once".to_owned(),
                    label: "second `.entry`".to_owned(),
                    related: Some((entries[0].0, Focus::Directive, "first `.entry`".to_owned())),
                });
                continue;
            }
            match by_name.get(name.as_str()) {
                Some(&(_, index)) => main_function = Some(index),
                None => errors.push(HeaderError {
                    directive: *position,
                    focus: Focus::Symbol,
                    message: format!("`.entry @{name}` names no `.func` directive"),
                    label: "no such function".to_owned(),
                    related: None,
                }),
            }
        }
        errors.extend(duplicates);

        // The string table: names, then parameters, then files, then string
        // constants; each string once, at its first occurrence.
        let mut strings = Interner::default();
        let names: Vec<u32> = functions
            .iter()
            .map(|(_, function, _)| strings.intern(&function.name))
            .collect();
        let params: Vec<Vec<u32>> = functions
            .iter()
            .map(|(_, function, _)| {
                function
                    .params
                    .iter()
                    .map(|(name, _)| strings.intern(name))
                    .collect()
            })
            .collect();
        let files: Vec<u32> = functions
            .iter()
            .map(|(_, _, file)| strings.intern(file))
            .collect();

        let mut pool = Vec::with_capacity(constants.len());
        for (position, constant) in constants {
            let value = match constant {
                ConstantDirective::Str(text) => {
                    let index = strings.intern(&text);
                    V::string(index).ok_or((
                        Focus::Argument,
                        "this value model has no string constants".to_owned(),
                        "not representable",
                    ))
                }
                ConstantDirective::Function(name) => match by_name.get(name.as_str()) {
                    Some(&(_, index)) => V::function(index).ok_or((
                        Focus::Argument,
                        "this value model has no function constants".to_owned(),
                        "not representable",
                    )),
                    None => Err((
                        Focus::Symbol,
                        format!("`.const fn @{name}` names no `.func` directive"),
                        "no such function",
                    )),
                },
                ConstantDirective::Value(value) => {
                    if value.string_index().is_some() {
                        Err((
                            Focus::Argument,
                            "string constants name their text: write `.const str \"...\"`"
                                .to_owned(),
                            "table index written by hand",
                        ))
                    } else if value.function_index().is_some() {
                        Err((
                            Focus::Argument,
                            "function constants name their function: write `.const fn @name`"
                                .to_owned(),
                            "table index written by hand",
                        ))
                    } else {
                        Ok(value)
                    }
                }
            };
            match value {
                Ok(value) => pool.push(value),
                Err((focus, message, label)) => errors.push(error(position, focus, message, label)),
            }
        }

        let mut seen = HashMap::new();
        let mut symbols = Vec::with_capacity(source_symbols.len());
        for (position, symbol) in source_symbols {
            if let Some(&first) = seen.get(&symbol.index) {
                errors.push(HeaderError {
                    directive: position,
                    focus: Focus::Argument,
                    message: format!("duplicate source symbol index {}", symbol.index),
                    label: "used again here".to_owned(),
                    related: Some((first, Focus::Argument, "first used here".to_owned())),
                });
            } else {
                seen.insert(symbol.index, position);
            }
            symbols.push(symbol);
        }

        if !errors.is_empty() {
            errors.sort_by_key(|error| error.directive);
            return Err(errors);
        }
        let functions = functions
            .into_iter()
            .enumerate()
            .map(|(index, (_, function, _))| FunctionDecl {
                name: names[index],
                signature: Signature {
                    params: function
                        .params
                        .into_iter()
                        .zip(&params[index])
                        .map(|((_, ty), &name)| Parameter { name, ty })
                        .collect(),
                    ret: function.ret,
                },
                local_count: function.locals,
                file: files[index],
            })
            .collect();
        Ok(ProgramMetadata {
            strings: strings.strings,
            functions,
            constants: pool,
            main_function,
            source_symbols: symbols,
            extra,
        })
    }

    /// [`assemble`](Self::assemble), failing with the first problem.
    pub fn finish<Info>(self, extra: Info) -> eyre::Result<ProgramMetadata<V, Ty, Info>>
    where
        V: ConstantValue,
    {
        self.assemble(extra).map_err(|errors| {
            let more = match errors.len() - 1 {
                0 => String::new(),
                1 => " (and 1 more problem)".to_owned(),
                n => format!(" (and {n} more problems)"),
            };
            eyre::eyre!("invalid header: {}{more}", errors[0])
        })
    }
}

#[cfg(test)]
mod tests;
