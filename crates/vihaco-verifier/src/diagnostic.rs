// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Structured verification errors and diagnostics, and how they render.
//!
//! A [`Diagnostic`] holds data, not text: types as [`Type`] and [`TypeSlot`]
//! values, table indices, the role of a mistyped value, the predecessors of a
//! join. Its `Display` works without the module and shows indices
//! (`struct 0`, `function 1, address 2`); [`Diagnostic::display_with`]
//! renders the same diagnostic with the module's names (`struct Point`,
//! `@main+2`) and the instruction text, given a [`DiagnosticNames`] such as
//! the module itself.

use std::fmt;

use vihaco_abi::Type;
use vihaco_abi::names::{InstructionText, SymbolNames, TypeNames};
use vihaco_module::module::{FrameError, LocalModule};

use crate::slot::TypeSlot;

/// How serious a [`Diagnostic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The program is accepted, but the finding is worth reporting.
    Warning,
    /// The program is rejected.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// What a mistyped value is to the instruction that rejected it, recorded in
/// a [`DiagnosticKind::TypeMismatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// Operand `n` of the instruction, numbered in the order its stack effect
    /// lists them: bottom first, so the last operand is the top of the stack.
    Operand(u32),
    /// Local `i`, whose declared type differs from the type a typed access
    /// (`load_u64 i`, `store_f64 i`) names.
    Local(u32),
    /// The value stored into local `i`.
    StoredValue(u32),
    /// Argument `index` of a call: to function `callee`, or to an indirect
    /// callee when `None`.
    Argument {
        /// The callee's function index, if the call is direct.
        callee: Option<u32>,
        /// The argument index, first argument 0.
        index: u32,
    },
    /// Return value `n` of the function being verified.
    Return(u32),
    /// Field `field` of struct `struct_id`, written by `struct_new` or
    /// `set_field`.
    Field {
        /// The struct-table index.
        struct_id: u32,
        /// The field slot.
        field: u32,
    },
}

/// What a [`DiagnosticKind::TypeMismatch`] required.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Expected {
    /// A value of this type (or one that satisfies it; see
    /// [`type_satisfies`](crate::type_satisfies)).
    Type(Type),
    /// An array of any element type.
    AnyArray,
    /// A value whose type every path agrees on.
    AnyKnown,
}

impl From<Type> for Expected {
    fn from(ty: Type) -> Self {
        Expected::Type(ty)
    }
}

impl From<&Type> for Expected {
    fn from(ty: &Type) -> Self {
        Expected::Type(ty.clone())
    }
}

/// How control reached a join point, recorded in a
/// [`DiagnosticKind::StackDepthMismatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Predecessor {
    /// The function's entry.
    Entry,
    /// Falling through from the instruction at this address.
    Fallthrough(u32),
    /// A jump from the instruction at this address.
    Jump(u32),
}

/// A metadata table an index refers to, in [`DiagnosticKind::UnknownEntry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Table {
    /// The function table.
    Functions,
    /// The struct table.
    Structs,
    /// The named-type table.
    NamedTypes,
    /// The constant table.
    Constants,
    /// The string table.
    Strings,
}

impl Table {
    fn entry(self) -> &'static str {
        match self {
            Table::Functions => "function",
            Table::Structs => "struct",
            Table::NamedTypes => "named type",
            Table::Constants => "constant",
            Table::Strings => "string",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Table::Functions => "function table",
            Table::Structs => "struct table",
            Table::NamedTypes => "named-type table",
            Table::Constants => "constant table",
            Table::Strings => "string table",
        }
    }
}

/// What a [`Diagnostic`] reports.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// An instruction needed more operands than the stack holds. A function
    /// may not reach into its locals or its caller's frame.
    StackUnderflow {
        /// How many operands the instruction needed.
        needed: usize,
        /// How many the stack held.
        depth: usize,
    },
    /// Two paths reach a join point with different operand depths. The join
    /// is the diagnostic's address.
    StackDepthMismatch {
        /// Depth already saved at the join point.
        expected: usize,
        /// Depth arriving along another edge.
        found: usize,
        /// The edge the saved depth arrived along.
        expected_from: Predecessor,
        /// The edge the conflicting depth arrived along.
        found_from: Predecessor,
    },
    /// A value of the wrong type was used.
    TypeMismatch {
        /// What the value is to the instruction, when the rule says.
        role: Option<Role>,
        /// What was required.
        expected: Expected,
        /// The abstract value found: a type, or a conflict where paths
        /// disagreed (with the join they disagree at).
        found: TypeSlot,
    },
    /// A call passes a different number of arguments than the callee declares.
    CallArityMismatch {
        /// The callee's function index.
        callee: u32,
        /// The callee's declared parameter count.
        declared: usize,
        /// The number of arguments passed.
        found: usize,
    },
    /// A return keeps a different number of values than the function declares.
    ReturnCountMismatch {
        /// The declared return count.
        declared: usize,
        /// The number of values returned.
        found: usize,
    },
    /// A local index outside the function's declared locals.
    LocalIndexOutOfRange {
        /// The index used.
        index: u32,
        /// The number of declared locals.
        count: u32,
    },
    /// Two paths reach a join point with different numbers of locals. A
    /// context that keeps the function's declared locals never produces this;
    /// it guards custom [`Frame`](crate::Frame) users.
    LocalCountMismatch {
        /// Local count saved at the join point.
        expected: usize,
        /// Local count arriving along another edge.
        found: usize,
    },
    /// A jump target outside the function containing the jump.
    InvalidControlFlowTarget {
        /// The offending target address.
        target: u32,
        /// First address of the function.
        start: u32,
        /// One past the function's last address.
        end: u32,
    },
    /// A [`Flow::Jump`](vihaco_abi::verify::Flow::Jump) with no targets. A
    /// jump must name at least one address; leaving the function is
    /// `Flow::Return`, stopping the machine `Flow::Halt`.
    EmptyJump,
    /// Some path runs off the end of the function without returning or
    /// halting.
    MissingTerminator,
    /// Instructions no path reaches, starting at this address.
    Unreachable,
    /// A function's span does not lie within the code it is verified against.
    InvalidFunctionSpan {
        /// First address of the span.
        start: u32,
        /// One past the last address of the span.
        end: u32,
        /// Number of instructions in the code.
        code_len: usize,
    },
    /// A function's declared frame is invalid (see
    /// [`FunctionInfo::check_frame`](vihaco_module::module::FunctionInfo::check_frame)).
    InvalidFrame(FrameError<Type>),
    /// An index outside a metadata table.
    UnknownEntry {
        /// The table.
        table: Table,
        /// The index used.
        index: u32,
        /// The table's length.
        len: usize,
    },
    /// A direct call by address names an address where no function starts.
    NoFunctionAt {
        /// The address called.
        address: u32,
    },
    /// A field slot past the end of a struct.
    UnknownField {
        /// The struct-table index.
        struct_id: u32,
        /// The field slot used.
        field: u32,
        /// The number of fields the struct declares.
        field_count: usize,
    },
    /// A named type looked up by a name the module does not declare.
    UndefinedNamedType {
        /// The name looked up.
        name: String,
    },
    /// `struct_new` carries a field count that disagrees with the struct
    /// table.
    FieldCountMismatch {
        /// The struct-table index.
        struct_id: u32,
        /// The number of fields the struct declares.
        declared: usize,
        /// The number of fields the instruction pops.
        found: usize,
    },
    /// The instruction cannot be verified statically: its operand count or
    /// shape comes from a runtime value (for example a count popped from the
    /// stack) rather than from an immediate operand. Device instructions should
    /// carry such counts as immediates; a rule reports this kind otherwise
    /// (see [`VerifyError::unverifiable`]).
    Unverifiable {
        /// Why the instruction cannot be verified.
        reason: String,
    },
    /// A component-defined error, from a rule or capability.
    Rule(String),
}

impl DiagnosticKind {
    /// The severity this kind is reported with: [`Unreachable`] is a
    /// [`Severity::Warning`], every other kind a [`Severity::Error`].
    ///
    /// [`Unreachable`]: DiagnosticKind::Unreachable
    pub fn severity(&self) -> Severity {
        match self {
            DiagnosticKind::Unreachable => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

/// Names for rendering diagnostics: the module's names (through
/// [`SymbolNames`]: functions, structs, fields, named types, labels,
/// strings), parameter names, function starts, and its instructions in source
/// syntax.
///
/// Implemented for [`LocalModule`]s whose instructions implement
/// [`InstructionText`], which every `#[composite]` instruction set does.
/// Implement it for other module models to render their diagnostics with
/// names. Verification itself needs none of this; only rendering does.
pub trait DiagnosticNames: SymbolNames {
    /// The address of the first instruction of function `function`.
    fn function_start(&self, function: u32) -> Option<u32>;

    /// The name of parameter `index` of function `function`.
    fn parameter_name(&self, function: u32, index: usize) -> Option<&str>;

    /// The instruction at `address` in source syntax, operands by name
    /// (`cpu::cpu::set_field @Point.y`).
    fn instruction_text(&self, address: u32) -> Option<String>;
}

impl<I: InstructionText, V, Ty, Info> DiagnosticNames for LocalModule<I, V, Ty, Info> {
    fn function_start(&self, function: u32) -> Option<u32> {
        Some(self.functions.get(function as usize)?.start_address)
    }

    fn parameter_name(&self, function: u32, index: usize) -> Option<&str> {
        self.names().parameter_name(function, index)
    }

    fn instruction_text(&self, address: u32) -> Option<String> {
        let instruction = self.code.get(address as usize)?;
        Some(instruction.text_with(&self.names()).to_string())
    }
}

/// One verification finding, located at an instruction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// The function being verified, as identified by its
    /// [`FunctionSpan::function`](crate::FunctionSpan::function).
    pub function: u32,
    /// The section-local instruction address the finding is about.
    pub address: u32,
    /// Whether the finding rejects the program.
    pub severity: Severity,
    /// What was found.
    pub kind: DiagnosticKind,
}

impl Diagnostic {
    /// A diagnostic with the severity of its kind (see
    /// [`DiagnosticKind::severity`]).
    pub fn new(function: u32, address: u32, kind: DiagnosticKind) -> Self {
        Self {
            function,
            address,
            severity: kind.severity(),
            kind,
        }
    }

    /// Whether this diagnostic rejects the program.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Render this diagnostic with the names in `names` (usually the module
    /// it was found in): functions as `@main`, locations as
    /// `@main+2 (address 2)` followed by the instruction, types and fields by
    /// name.
    ///
    /// ```text
    /// error: operand 0 of `cpu::cpu::get_field`: expected struct Point, found struct Pair
    ///   --> @main+2 (address 2): cpu::cpu::get_field @Point.y
    /// ```
    pub fn display_with<'a>(&'a self, names: &'a dyn DiagnosticNames) -> impl fmt::Display + 'a {
        Rendered {
            diagnostic: self,
            names: Some(names),
        }
    }

    /// The headline message alone, without severity or location, rendered
    /// with `names` if given.
    pub fn message<'a>(&'a self, names: Option<&'a dyn DiagnosticNames>) -> impl fmt::Display + 'a {
        Message(Renderer {
            diagnostic: self,
            names,
        })
    }
}

/// Two lines: `error: <message>` and `  --> function 0, address 2`.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Rendered {
            diagnostic: self,
            names: None,
        }
        .fmt(f)
    }
}

struct Rendered<'a> {
    diagnostic: &'a Diagnostic,
    names: Option<&'a dyn DiagnosticNames>,
}

impl fmt::Display for Rendered<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let renderer = Renderer {
            diagnostic: self.diagnostic,
            names: self.names,
        };
        write!(f, "{}: ", self.diagnostic.severity)?;
        renderer.message(f)?;
        f.write_str("\n  --> ")?;
        renderer.location(f, self.diagnostic.address)?;
        if let Some(instruction) = renderer.instruction(self.diagnostic.address) {
            write!(f, ": {instruction}")?;
        }
        Ok(())
    }
}

struct Message<'a>(Renderer<'a>);

impl fmt::Display for Message<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.message(f)
    }
}

/// Renders the parts of one diagnostic, with or without names.
#[derive(Clone, Copy)]
struct Renderer<'a> {
    diagnostic: &'a Diagnostic,
    names: Option<&'a dyn DiagnosticNames>,
}

impl<'a> Renderer<'a> {
    fn type_names(&self) -> &'a dyn TypeNames {
        match self.names {
            Some(names) => names_as_types(names),
            None => &(),
        }
    }

    fn ty(&self, ty: &Type) -> String {
        ty.display_with(self.type_names()).to_string()
    }

    fn instruction(&self, address: u32) -> Option<String> {
        self.names?.instruction_text(address)
    }

    /// `@main` or `function 0`.
    fn function(&self, function: u32) -> String {
        match self.names.and_then(|names| names.function_name(function)) {
            Some(name) => format!("@{name}"),
            None => format!("function {function}"),
        }
    }

    /// `@main+2` or `address 2`, inside the diagnostic's function.
    fn short_location(&self, address: u32) -> String {
        let function = self.diagnostic.function;
        match self.names.and_then(|names| {
            let name = names.function_name(function)?;
            let offset = address.checked_sub(names.function_start(function)?)?;
            Some((name, offset))
        }) {
            Some((name, offset)) => format!("@{name}+{offset}"),
            None => format!("address {address}"),
        }
    }

    /// `@main+2 (address 2)`, or `function 0, address 2` without names.
    fn location(&self, f: &mut fmt::Formatter<'_>, address: u32) -> fmt::Result {
        let short = self.short_location(address);
        if short.starts_with('@') {
            write!(f, "{short} (address {address})")
        } else {
            write!(
                f,
                "function {}, address {address}",
                self.diagnostic.function
            )
        }
    }

    /// The instruction's mnemonic in backticks, or `the instruction`.
    fn who(&self) -> String {
        match self.instruction(self.diagnostic.address) {
            Some(text) => {
                let mnemonic = text.split_whitespace().next().unwrap_or_default();
                format!("`{mnemonic}`")
            }
            None => "the instruction".to_owned(),
        }
    }

    fn expected(&self, expected: &Expected) -> String {
        match expected {
            Expected::Type(ty) => self.ty(ty),
            Expected::AnyArray => "an array".to_owned(),
            Expected::AnyKnown => "a value of a known type".to_owned(),
        }
    }

    fn found(&self, found: &TypeSlot) -> String {
        match found {
            TypeSlot::Known(ty) => self.ty(ty),
            TypeSlot::Conflict {
                first,
                second,
                join,
            } => {
                let join = match join {
                    Some(address) => format!(" at join {}", self.short_location(*address)),
                    None => String::new(),
                };
                format!(
                    "{} or {} (paths disagree{join})",
                    self.ty(first),
                    self.ty(second)
                )
            }
        }
    }

    fn predecessor(&self, predecessor: Predecessor) -> String {
        match predecessor {
            Predecessor::Entry => "the function entry".to_owned(),
            Predecessor::Fallthrough(address) => {
                format!("{} (falling through)", self.short_location(address))
            }
            Predecessor::Jump(address) => match self.instruction(address) {
                Some(instruction) => format!("{} ({instruction})", self.short_location(address)),
                None => format!("{} (a jump)", self.short_location(address)),
            },
        }
    }

    fn struct_field(&self, struct_id: u32, field: u32) -> String {
        let names = self.names;
        match (
            names.and_then(|names| names.struct_name(struct_id)),
            names.and_then(|names| names.field_name(struct_id, field)),
        ) {
            (Some(name), Some(field)) => format!("field {name}.{field}"),
            _ => format!("field {field} of {}", self.ty(&Type::Struct(struct_id))),
        }
    }

    fn message(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let function = self.diagnostic.function;
        match &self.diagnostic.kind {
            DiagnosticKind::StackUnderflow { needed, depth } => write!(
                f,
                "stack underflow: {} needs {needed} operand{}, but the stack has {depth}",
                self.who(),
                plural(*needed)
            ),
            DiagnosticKind::StackDepthMismatch {
                expected,
                found,
                expected_from,
                found_from,
            } => write!(
                f,
                "operand stack depth differs where paths join: {expected} from {}, {found} from {}",
                self.predecessor(*expected_from),
                self.predecessor(*found_from)
            ),
            DiagnosticKind::TypeMismatch {
                role,
                expected,
                found,
            } => self.type_mismatch(f, *role, expected, found),
            DiagnosticKind::CallArityMismatch {
                callee,
                declared,
                found,
            } => write!(
                f,
                "call to {} passes {found} argument{}, but it declares {declared}",
                self.function(*callee),
                plural(*found)
            ),
            DiagnosticKind::ReturnCountMismatch { declared, found } => write!(
                f,
                "return keeps {found} value{}, but {} declares {declared} result{}",
                plural(*found),
                self.function(function),
                plural(*declared)
            ),
            DiagnosticKind::LocalIndexOutOfRange { index, count } => write!(
                f,
                "local {index} does not exist: {} declares {count} local{}",
                self.function(function),
                plural(*count as usize)
            ),
            DiagnosticKind::LocalCountMismatch { expected, found } => write!(
                f,
                "local count differs where paths join: {expected} saved, {found} arriving"
            ),
            DiagnosticKind::InvalidControlFlowTarget { target, start, end } => write!(
                f,
                "jump target {target} is outside {} (addresses {start}..{end})",
                self.function(function)
            ),
            DiagnosticKind::EmptyJump => write!(
                f,
                "{} jumps, but names no target address (use a return or halt to end a path)",
                self.who()
            ),
            DiagnosticKind::MissingTerminator => write!(
                f,
                "control falls off the end of {} without a return or halt",
                self.function(function)
            ),
            DiagnosticKind::Unreachable => f.write_str("unreachable code"),
            DiagnosticKind::InvalidFunctionSpan {
                start,
                end,
                code_len,
            } => write!(
                f,
                "{} spans addresses {start}..{end}, but the code has {code_len} instructions",
                self.function(function)
            ),
            DiagnosticKind::InvalidFrame(error) => {
                let names = self.names;
                let shown = error.display_with(
                    move |index| names?.parameter_name(function, index),
                    |ty| self.ty(ty),
                );
                write!(f, "invalid frame for {}: {shown}", self.function(function))
            }
            DiagnosticKind::UnknownEntry { table, index, len } => write!(
                f,
                "{} {index} is outside the {} (length {len})",
                table.entry(),
                table.name()
            ),
            DiagnosticKind::NoFunctionAt { address } => {
                write!(f, "no function starts at address {address}")
            }
            DiagnosticKind::UnknownField {
                struct_id,
                field,
                field_count,
            } => write!(
                f,
                "{} has no field {field} (it has {field_count})",
                self.ty(&Type::Struct(*struct_id))
            ),
            DiagnosticKind::UndefinedNamedType { name } => {
                write!(f, "undefined named type `@{name}`")
            }
            DiagnosticKind::FieldCountMismatch {
                struct_id,
                declared,
                found,
            } => write!(
                f,
                "{} pops {found} field{}, but {} declares {declared}",
                self.who(),
                plural(*found),
                self.ty(&Type::Struct(*struct_id))
            ),
            DiagnosticKind::Unverifiable { reason } => {
                write!(f, "instruction cannot be verified: {reason}")
            }
            DiagnosticKind::Rule(message) => f.write_str(message),
        }
    }

    fn type_mismatch(
        &self,
        f: &mut fmt::Formatter<'_>,
        role: Option<Role>,
        expected: &Expected,
        found: &TypeSlot,
    ) -> fmt::Result {
        let (expected, found_text) = (self.expected(expected), self.found(found));
        match role {
            None => write!(f, "type mismatch: expected {expected}, found {found_text}"),
            Some(Role::Operand(n)) => write!(
                f,
                "operand {n} of {}: expected {expected}, found {found_text}",
                self.who()
            ),
            Some(Role::Local(index)) => write!(
                f,
                "local {index} is declared {found_text}, but {} accesses it as {expected} \
                 (use the access for {found_text}, or the untyped one)",
                self.who()
            ),
            Some(Role::StoredValue(index)) => write!(
                f,
                "value stored to local {index}: expected {expected}, found {found_text}"
            ),
            Some(Role::Argument { callee, index }) => {
                write!(f, "argument {index}")?;
                if let Some(callee) = callee {
                    if let Some(name) = self
                        .names
                        .and_then(|names| names.parameter_name(callee, index as usize))
                    {
                        write!(f, " ({name})")?;
                    }
                    write!(f, " of {}", self.function(callee))?;
                } else {
                    f.write_str(" of the indirect call")?;
                }
                write!(f, ": expected {expected}, found {found_text}")
            }
            Some(Role::Return(n)) => write!(
                f,
                "return value {n} of {}: expected {expected}, found {found_text}",
                self.function(self.diagnostic.function)
            ),
            Some(Role::Field { struct_id, field }) => write!(
                f,
                "{}: expected {expected}, found {found_text}",
                self.struct_field(struct_id, field)
            ),
        }
    }
}

/// Upcast, since trait upcasting coercion from `dyn DiagnosticNames` to
/// `dyn TypeNames` is implicit.
fn names_as_types(names: &dyn DiagnosticNames) -> &dyn TypeNames {
    names
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// A structured error raised by a capability or rule.
///
/// Rules return `eyre::Result`; a `VerifyError` converts into an
/// [`eyre::Report`] with `?`, and the engine recovers its [`DiagnosticKind`]
/// by downcasting. Any other error a rule returns is reported as
/// [`DiagnosticKind::Rule`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VerifyError {
    kind: DiagnosticKind,
}

impl VerifyError {
    /// An error of the given kind.
    pub fn new(kind: DiagnosticKind) -> Self {
        Self { kind }
    }

    /// A [`DiagnosticKind::StackUnderflow`]: the instruction needed `needed`
    /// operands and found `depth`.
    pub fn stack_underflow(needed: usize, depth: usize) -> Self {
        Self::new(DiagnosticKind::StackUnderflow { needed, depth })
    }

    /// A [`DiagnosticKind::TypeMismatch`] between what was required and the
    /// abstract value found, with no role; add one with
    /// [`with_role`](VerifyError::with_role).
    pub fn type_mismatch(expected: impl Into<Expected>, found: TypeSlot) -> Self {
        Self::new(DiagnosticKind::TypeMismatch {
            role: None,
            expected: expected.into(),
            found,
        })
    }

    /// Record what the mistyped value is to the instruction, if this is a
    /// [`DiagnosticKind::TypeMismatch`] without a role yet; any other error
    /// is returned unchanged.
    ///
    /// ```
    /// # use vihaco_verifier::{DiagnosticKind, Role, Type, TypeSlot, VerifyError};
    /// let error = VerifyError::type_mismatch(Type::U64, TypeSlot::Known(Type::F64))
    ///     .with_role(Role::Operand(1));
    /// assert_eq!(error.to_string(), "operand 1 of the instruction: expected u64, found f64");
    /// ```
    pub fn with_role(mut self, role: Role) -> Self {
        if let DiagnosticKind::TypeMismatch {
            role: slot @ None, ..
        } = &mut self.kind
        {
            *slot = Some(role);
        }
        self
    }

    /// A [`DiagnosticKind::CallArityMismatch`]: a call to `callee` passes
    /// `found` arguments where it declares `declared` parameters.
    pub fn call_arity_mismatch(callee: u32, declared: usize, found: usize) -> Self {
        Self::new(DiagnosticKind::CallArityMismatch {
            callee,
            declared,
            found,
        })
    }

    /// A [`DiagnosticKind::ReturnCountMismatch`]: a return keeps `found`
    /// values where the function declares `declared` results.
    pub fn return_count_mismatch(declared: usize, found: usize) -> Self {
        Self::new(DiagnosticKind::ReturnCountMismatch { declared, found })
    }

    /// A [`DiagnosticKind::LocalIndexOutOfRange`]: local `index` of a function
    /// declaring `count` locals.
    pub fn local_index_out_of_range(index: u32, count: u32) -> Self {
        Self::new(DiagnosticKind::LocalIndexOutOfRange { index, count })
    }

    /// A [`DiagnosticKind::UnknownEntry`]: `index` is outside `table`, of
    /// length `len`.
    pub fn unknown_entry(table: Table, index: u32, len: usize) -> Self {
        Self::new(DiagnosticKind::UnknownEntry { table, index, len })
    }

    /// A [`DiagnosticKind::Unverifiable`] error, for a rule whose instruction
    /// takes its operand count or shape from a runtime value, so its stack
    /// effect cannot be known before execution:
    ///
    /// ```
    /// # use vihaco_verifier::{DiagnosticKind, VerifyError};
    /// let error = VerifyError::unverifiable("`pulse_n` pops its count from the stack");
    /// assert!(matches!(error.kind(), DiagnosticKind::Unverifiable { .. }));
    /// ```
    ///
    /// Prefer carrying the count as an immediate (`pulse_n 4`), which a rule
    /// can check.
    pub fn unverifiable(reason: impl fmt::Display) -> Self {
        Self::new(DiagnosticKind::Unverifiable {
            reason: reason.to_string(),
        })
    }

    /// A component-defined [`DiagnosticKind::Rule`] error.
    pub fn rule(message: impl fmt::Display) -> Self {
        Self::new(DiagnosticKind::Rule(message.to_string()))
    }

    /// What went wrong.
    pub fn kind(&self) -> &DiagnosticKind {
        &self.kind
    }

    /// Consume the error, returning its kind.
    pub fn into_kind(self) -> DiagnosticKind {
        self.kind
    }
}

impl From<DiagnosticKind> for VerifyError {
    fn from(kind: DiagnosticKind) -> Self {
        Self::new(kind)
    }
}

/// The diagnostic message without names, as [`Diagnostic::message`] renders
/// it.
impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Located nowhere in particular: only the message is shown.
        let diagnostic = Diagnostic::new(0, 0, self.kind.clone());
        diagnostic.message(None).fmt(f)
    }
}

impl std::error::Error for VerifyError {}

/// The diagnostic kind for an error returned by a rule: the structured kind
/// of a [`VerifyError`] (also when wrapped in context), otherwise
/// [`DiagnosticKind::Rule`] with the error's message chain.
pub(crate) fn classify(report: &eyre::Report) -> DiagnosticKind {
    match report.downcast_ref::<VerifyError>() {
        Some(error) => error.kind.clone(),
        None => DiagnosticKind::Rule(format!("{report:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_abi::Value;
    use vihaco_module::module::{FieldInfo, FunctionInfo, Parameter, Signature, StructInfo};

    #[test]
    fn reports_round_trip_through_eyre() {
        let report: eyre::Report = VerifyError::stack_underflow(2, 0).into();
        assert_eq!(
            classify(&report),
            DiagnosticKind::StackUnderflow {
                needed: 2,
                depth: 0
            }
        );
        let report = eyre::eyre!("channel 3 is not open");
        assert_eq!(
            classify(&report),
            DiagnosticKind::Rule("channel 3 is not open".into())
        );
    }

    #[test]
    fn only_unreachable_is_a_warning() {
        assert_eq!(DiagnosticKind::Unreachable.severity(), Severity::Warning);
        assert_eq!(
            DiagnosticKind::MissingTerminator.severity(),
            Severity::Error
        );
        let unverifiable = VerifyError::unverifiable("count from the stack");
        assert_eq!(unverifiable.kind().severity(), Severity::Error);
        assert_eq!(
            unverifiable.to_string(),
            "instruction cannot be verified: count from the stack"
        );
        let d = Diagnostic::new(
            2,
            7,
            DiagnosticKind::StackUnderflow {
                needed: 1,
                depth: 0,
            },
        );
        assert!(d.is_error());
        assert_eq!(
            d.to_string(),
            "error: stack underflow: the instruction needs 1 operand, but the stack has 0\n  \
             --> function 2, address 7"
        );
    }

    /// `main(p0: u64) -> f64` at 0..3 and `helper(p0: f64)` at 3..4, with
    /// `Point { x: i64, y: f64 }` and `Pair { a: u64 }`.
    fn module() -> LocalModule<&'static str, Value, Type> {
        let function = |name, param: Type, ret: Vec<Type>, start, end| FunctionInfo {
            name,
            signature: Signature {
                params: vec![Parameter {
                    name: 2,
                    ty: param.clone(),
                }],
                ret,
            },
            locals: vec![param],
            start_address: start,
            end_address: end,
            file: 0,
        };
        let field = |name, ty| FieldInfo { name, ty };
        LocalModule {
            code: vec!["cpu.add_u64", "cpu.get_field 0, 1", "br 1", "ret 0"],
            functions: vec![
                function(0, Type::U64, vec![Type::F64], 0, 3),
                function(1, Type::F64, vec![], 3, 4),
            ],
            structs: vec![
                StructInfo {
                    name: 3,
                    fields: vec![field(4, Type::I64), field(5, Type::F64)],
                },
                StructInfo {
                    name: 6,
                    fields: vec![field(7, Type::U64)],
                },
            ],
            strings: ["main", "helper", "p0", "Point", "x", "y", "Pair", "a"]
                .map(String::from)
                .to_vec(),
            ..LocalModule::default()
        }
    }

    fn render(function: u32, address: u32, kind: DiagnosticKind) -> (String, String) {
        let module = module();
        let diagnostic = Diagnostic::new(function, address, kind);
        (
            diagnostic.display_with(&module).to_string(),
            diagnostic.to_string(),
        )
    }

    #[test]
    fn type_mismatches_name_their_types_role_and_location() {
        let kind = VerifyError::type_mismatch(Type::Struct(0), TypeSlot::Known(Type::Struct(1)))
            .with_role(Role::Operand(0))
            .into_kind();
        let (named, plain) = render(0, 1, kind);
        assert_eq!(
            named,
            "error: operand 0 of `cpu.get_field`: expected struct Point, found struct Pair\n  \
             --> @main+1 (address 1): cpu.get_field 0, 1"
        );
        assert_eq!(
            plain,
            "error: operand 0 of the instruction: expected struct 0, found struct 1\n  \
             --> function 0, address 1"
        );

        let mismatch = |role, expected: Type, found: Type| {
            VerifyError::type_mismatch(expected, TypeSlot::Known(found))
                .with_role(role)
                .into_kind()
        };
        let message = |function, address, kind| {
            let module = module();
            Diagnostic::new(function, address, kind)
                .message(Some(&module))
                .to_string()
        };
        assert_eq!(
            message(0, 0, mismatch(Role::Local(0), Type::F64, Type::U64)),
            "local 0 is declared u64, but `cpu.add_u64` accesses it as f64 (use the access for \
             u64, or the untyped one)"
        );
        assert_eq!(
            message(
                0,
                0,
                mismatch(
                    Role::Argument {
                        callee: Some(1),
                        index: 0
                    },
                    Type::F64,
                    Type::U64
                )
            ),
            "argument 0 (p0) of @helper: expected f64, found u64"
        );
        assert_eq!(
            message(
                0,
                0,
                mismatch(
                    Role::Field {
                        struct_id: 0,
                        field: 0
                    },
                    Type::I64,
                    Type::U64
                )
            ),
            "field Point.x: expected i64, found u64"
        );
        assert_eq!(
            message(0, 2, mismatch(Role::Return(0), Type::F64, Type::U64)),
            "return value 0 of @main: expected f64, found u64"
        );
        assert_eq!(
            message(0, 2, mismatch(Role::StoredValue(0), Type::U64, Type::I64)),
            "value stored to local 0: expected u64, found i64"
        );
        let conflict = TypeSlot::Conflict {
            first: Type::U64,
            second: Type::F64,
            join: Some(2),
        };
        assert_eq!(
            message(
                0,
                2,
                VerifyError::type_mismatch(Type::U64, conflict).into_kind()
            ),
            "type mismatch: expected u64, found u64 or f64 (paths disagree at join @main+2)"
        );
    }

    #[test]
    fn structural_diagnostics_name_functions_ranges_and_paths() {
        let (named, plain) = render(
            0,
            2,
            DiagnosticKind::StackDepthMismatch {
                expected: 1,
                found: 2,
                expected_from: Predecessor::Jump(2),
                found_from: Predecessor::Fallthrough(1),
            },
        );
        assert_eq!(
            named,
            "error: operand stack depth differs where paths join: 1 from @main+2 (br 1), 2 from \
             @main+1 (falling through)\n  --> @main+2 (address 2): br 1"
        );
        assert!(
            plain.starts_with(
                "error: operand stack depth differs where paths join: 1 from address 2 (a jump), \
                 2 from address 1 (falling through)"
            ),
            "{plain}"
        );
        let (named, _) = render(
            0,
            2,
            DiagnosticKind::InvalidControlFlowTarget {
                target: 99,
                start: 0,
                end: 3,
            },
        );
        assert!(
            named.starts_with("error: jump target 99 is outside @main (addresses 0..3)"),
            "{named}"
        );
        let (named, _) = render(
            0,
            0,
            DiagnosticKind::StackUnderflow {
                needed: 2,
                depth: 0,
            },
        );
        assert!(
            named.starts_with(
                "error: stack underflow: `cpu.add_u64` needs 2 operands, but the stack has 0"
            ),
            "{named}"
        );
        let (named, _) = render(
            0,
            0,
            DiagnosticKind::CallArityMismatch {
                callee: 1,
                declared: 1,
                found: 2,
            },
        );
        assert!(
            named.starts_with("error: call to @helper passes 2 arguments, but it declares 1"),
            "{named}"
        );
        let frame = FrameError::LocalDisagreesWithParameter {
            index: 0,
            local: Type::Struct(1),
            parameter: Type::Struct(0),
        };
        let (named, plain) = render(1, 3, DiagnosticKind::InvalidFrame(frame));
        assert!(
            named.starts_with(
                "error: invalid frame for @helper: local 0 is declared struct Pair, but \
                 parameter 0 (p0) is struct Point"
            ),
            "{named}"
        );
        assert!(
            plain.starts_with(
                "error: invalid frame for function 1: local 0 is declared struct 1, but \
                 parameter 0 is struct 0"
            ),
            "{plain}"
        );
        let (named, _) = render(
            0,
            1,
            DiagnosticKind::UnknownField {
                struct_id: 1,
                field: 3,
                field_count: 1,
            },
        );
        assert!(
            named.starts_with("error: struct Pair has no field 3 (it has 1)"),
            "{named}"
        );
        let (named, _) = render(
            0,
            1,
            DiagnosticKind::UnknownEntry {
                table: Table::Functions,
                index: 5,
                len: 2,
            },
        );
        assert!(
            named.starts_with("error: function 5 is outside the function table (length 2)"),
            "{named}"
        );
    }
}
