// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Structured verification errors and diagnostics.

use std::fmt;

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

/// What a [`Diagnostic`] reports.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// An instruction popped an empty operand stack. A function may not reach
    /// into its locals or its caller's frame.
    StackUnderflow,
    /// Two paths reach a join point with different operand depths. `expected`
    /// is the depth already saved at the join, `found` the incoming depth.
    StackDepthMismatch {
        /// Depth saved at the join point.
        expected: usize,
        /// Depth arriving along another edge.
        found: usize,
    },
    /// A value of the wrong type was used.
    TypeMismatch {
        /// The required type.
        expected: String,
        /// The abstract value found, as displayed by its slot.
        found: String,
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

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticKind::StackUnderflow => write!(f, "operand stack underflow"),
            DiagnosticKind::StackDepthMismatch { expected, found } => write!(
                f,
                "operand stack depth mismatch at join: expected {expected}, found {found}"
            ),
            DiagnosticKind::TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {expected}, found {found}")
            }
            DiagnosticKind::CallArityMismatch {
                callee,
                declared,
                found,
            } => write!(
                f,
                "call to function {callee} passes {found} arguments, but it declares {declared}"
            ),
            DiagnosticKind::ReturnCountMismatch { declared, found } => write!(
                f,
                "return keeps {found} values, but the function declares {declared}"
            ),
            DiagnosticKind::LocalIndexOutOfRange { index, count } => write!(
                f,
                "local index {index} out of range: the function declares {count} locals"
            ),
            DiagnosticKind::LocalCountMismatch { expected, found } => write!(
                f,
                "local count mismatch at join: expected {expected}, found {found}"
            ),
            DiagnosticKind::InvalidControlFlowTarget { target } => {
                write!(f, "control-flow target {target} is outside the function")
            }
            DiagnosticKind::EmptyJump => {
                write!(f, "the instruction jumps, but names no target address")
            }
            DiagnosticKind::MissingTerminator => {
                write!(f, "control falls off the end of the function")
            }
            DiagnosticKind::Unreachable => write!(f, "unreachable code"),
            DiagnosticKind::InvalidFunctionSpan {
                start,
                end,
                code_len,
            } => write!(
                f,
                "function span {start}..{end} does not fit in {code_len} instructions"
            ),
            DiagnosticKind::Unverifiable { reason } => {
                write!(f, "instruction cannot be verified: {reason}")
            }
            DiagnosticKind::Rule(message) => f.write_str(message),
        }
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
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: function {} at {}: {}",
            self.severity, self.function, self.address, self.kind
        )
    }
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

    /// A [`DiagnosticKind::StackUnderflow`].
    pub fn stack_underflow() -> Self {
        Self::new(DiagnosticKind::StackUnderflow)
    }

    /// A [`DiagnosticKind::TypeMismatch`] between a required type and the
    /// abstract value found.
    pub fn type_mismatch(expected: impl fmt::Display, found: impl fmt::Display) -> Self {
        Self::new(DiagnosticKind::TypeMismatch {
            expected: expected.to_string(),
            found: found.to_string(),
        })
    }

    /// A [`DiagnosticKind::StackDepthMismatch`].
    pub fn stack_depth_mismatch(expected: usize, found: usize) -> Self {
        Self::new(DiagnosticKind::StackDepthMismatch { expected, found })
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

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
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

    #[test]
    fn reports_round_trip_through_eyre() {
        let report: eyre::Report = VerifyError::stack_underflow().into();
        assert_eq!(classify(&report), DiagnosticKind::StackUnderflow);
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
        let unverifiable = VerifyError::unverifiable("count from the stack").into_kind();
        assert_eq!(unverifiable.severity(), Severity::Error);
        assert_eq!(
            unverifiable.to_string(),
            "instruction cannot be verified: count from the stack"
        );
        let d = Diagnostic::new(2, 7, DiagnosticKind::StackUnderflow);
        assert!(d.is_error());
        assert_eq!(
            d.to_string(),
            "error: function 2 at 7: operand stack underflow"
        );
    }
}
