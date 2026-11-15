// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The verification engine against small hand-written fixture dialects.

use std::cell::Cell;
use std::collections::BTreeSet;

use vihaco_verifier::{
    AbstractState, Diagnostic, DiagnosticKind, Flow, Frame, FrameState, FunctionSpan, OperandStack,
    Rejection, Severity, Type, TypeSlot, TypedSlot, Verification, Verify, VerifyError,
    VerifyOptions, stack_effect, verify_functions,
};

thread_local! {
    /// How many times a `Fail` rule has run, to observe convergence.
    static FAIL_RUNS: Cell<usize> = const { Cell::new(0) };
}

/// A fixture stack dialect.
#[derive(Clone, Debug)]
enum Op {
    /// Push a value of the given type.
    Const(Type),
    /// Pop one value of any type.
    Pop,
    /// Pop `n` values of any type.
    PopN(usize),
    /// Pop a value that must have the given type.
    Use(Type),
    /// `[i64, i64] -> [i64]`.
    Add,
    /// Unconditional branch.
    Br(u32),
    /// Pop a `bool`, then branch to either target.
    CondBr(u32, u32),
    /// Leave the function.
    Ret,
    /// Stop the machine.
    Halt,
    /// A rule that always fails with a component-defined message.
    Fail(&'static str),
    /// A jump table: continue at any listed address (possibly none).
    Table(Vec<u32>),
}

impl<C> Verify<C> for Op
where
    C: OperandStack + ?Sized,
    C::Slot: TypedSlot,
{
    fn flow(&self) -> Flow {
        match self {
            Op::Br(target) => Flow::jump(*target),
            Op::CondBr(then, otherwise) => Flow::jump_to([*then, *otherwise]),
            Op::Table(targets) => Flow::jump_to(targets.iter().copied()),
            Op::Ret => Flow::Return,
            Op::Halt => Flow::Halt,
            _ => Flow::Next,
        }
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        match self {
            Op::Const(ty) => cx.push(C::Slot::of_type(ty.clone())),
            Op::Pop => {
                cx.pop()?;
            }
            Op::PopN(n) => {
                cx.pop_n(*n)?;
            }
            Op::Use(ty) => {
                cx.pop_expect(ty)?;
            }
            Op::Add => stack_effect(cx, [Type::I64, Type::I64], [Type::I64])?,
            Op::CondBr(..) => {
                cx.pop_expect(&Type::Bool)?;
            }
            Op::Br(_) | Op::Ret | Op::Halt | Op::Table(_) => {}
            Op::Fail(message) => {
                FAIL_RUNS.with(|runs| runs.set(runs.get() + 1));
                eyre::bail!("{message}");
            }
        }
        Ok(())
    }
}

use Op::*;

type Outcome = Result<Verification<Frame<TypeSlot>>, Rejection>;

fn span(function: u32, start: u32, end: u32) -> FunctionSpan<Frame<TypeSlot>> {
    FunctionSpan {
        function,
        start,
        end,
        entry: Frame::new(vec![]),
    }
}

fn verify_spans(code: &[Op], spans: &[FunctionSpan<Frame<TypeSlot>>], type_maps: bool) -> Outcome {
    let mut cx = FrameState::new(Frame::new(vec![]));
    let options = VerifyOptions::default().with_type_maps(type_maps);
    verify_functions(code, spans, &mut cx, options)
}

/// Verify `code` as a single function 0 spanning all of it.
fn verify(code: &[Op]) -> Outcome {
    verify_spans(code, &[span(0, 0, code.len() as u32)], false)
}

/// `(function, address, kind)` of every diagnostic.
fn found(diagnostics: &[Diagnostic]) -> Vec<(u32, u32, DiagnosticKind)> {
    diagnostics
        .iter()
        .map(|d| (d.function, d.address, d.kind.clone()))
        .collect()
}

fn rejected(outcome: Outcome) -> Vec<(u32, u32, DiagnosticKind)> {
    match outcome {
        Ok(verification) => panic!("expected a rejection, got {verification:?}"),
        Err(rejection) => found(&rejection.diagnostics),
    }
}

fn accepted(outcome: Outcome) -> Verification<Frame<TypeSlot>> {
    outcome.unwrap_or_else(|rejection| panic!("{rejection}"))
}

fn mismatch(expected: &str, found: &str) -> DiagnosticKind {
    DiagnosticKind::TypeMismatch {
        expected: expected.into(),
        found: found.into(),
    }
}

/// `if cond { push a } else { push b }; use(c); ret`.
fn diamond(a: Type, b: Type, then_use: Op) -> Vec<Op> {
    vec![
        Const(Type::Bool), // 0
        CondBr(2, 4),      // 1
        Const(a),          // 2
        Br(5),             // 3
        Const(b),          // 4
        then_use,          // 5 (join)
        Ret,               // 6
    ]
}

#[test]
fn a_balanced_diamond_is_clean() {
    let verification = accepted(verify(&diamond(Type::I64, Type::I64, Use(Type::I64))));
    assert!(verification.warnings.is_empty());
    assert!(
        verification.type_maps.is_none(),
        "type maps are off by default"
    );
}

#[test]
fn an_unbalanced_diamond_is_one_depth_mismatch_at_the_join() {
    let code = [
        Const(Type::Bool),
        CondBr(2, 5),
        Const(Type::I64),
        Const(Type::I64),
        Br(6),
        Const(Type::I64),
        Ret, // 6: join, reached with depth 2 then 1
    ];
    assert_eq!(
        rejected(verify(&code)),
        [(
            0,
            6,
            DiagnosticKind::StackDepthMismatch {
                expected: 2,
                found: 1
            }
        )]
    );
}

#[test]
fn a_zero_delta_loop_converges() {
    let code = [
        Const(Type::I64),  // 0: loop-carried value
        Const(Type::Bool), // 1: header
        CondBr(3, 6),      // 2
        Use(Type::I64),    // 3: body consumes it...
        Const(Type::I64),  // 4: ...and replaces it
        Br(1),             // 5: back edge
        Ret,               // 6
    ];
    accepted(verify(&code));
}

#[test]
fn a_nonzero_delta_loop_is_a_mismatch_at_the_header() {
    let code = [
        Const(Type::I64),  // 0
        Const(Type::Bool), // 1: header, entered at depth 1
        CondBr(3, 5),      // 2
        Const(Type::I64),  // 3: body pushes one per iteration
        Br(1),             // 4: back edge at depth 2
        Ret,               // 5
    ];
    assert_eq!(
        rejected(verify(&code)),
        [(
            0,
            1,
            DiagnosticKind::StackDepthMismatch {
                expected: 1,
                found: 2
            }
        )]
    );
}

#[test]
fn unreachable_code_is_only_a_warning() {
    let code = [Ret, Const(Type::I64), Fail("never runs"), Ret];
    let verification = accepted(verify(&code));
    assert_eq!(
        found(&verification.warnings),
        [(0, 1, DiagnosticKind::Unreachable)],
        "one warning per unreachable run, and dead code is not verified"
    );
    assert_eq!(verification.warnings[0].severity, Severity::Warning);
}

#[test]
fn falling_off_the_end_is_a_missing_terminator() {
    let code = [Const(Type::I64), Pop];
    assert_eq!(
        rejected(verify(&code)),
        [(0, 1, DiagnosticKind::MissingTerminator)]
    );
    assert_eq!(
        rejected(verify_spans(&[], &[span(4, 0, 0)], false)),
        [(4, 0, DiagnosticKind::MissingTerminator)]
    );
}

#[test]
fn jumping_out_of_the_function_is_an_invalid_target() {
    // Function 0 is 0..3; its branch reaches into function 1 at 3..5.
    let code = [Const(Type::Bool), CondBr(2, 3), Ret, Ret, Ret];
    let outcome = verify_spans(&code, &[span(0, 0, 3), span(1, 3, 5)], false);
    let rejection = outcome.unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 1, DiagnosticKind::InvalidControlFlowTarget { target: 3 }),
            (1, 4, DiagnosticKind::Unreachable),
        ]
    );
    assert_eq!(rejection.errors().count(), 1);
}

#[test]
fn invalid_function_spans_are_reported() {
    assert_eq!(
        rejected(verify_spans(&[Ret], &[span(0, 0, 4)], false)),
        [(
            0,
            0,
            DiagnosticKind::InvalidFunctionSpan {
                start: 0,
                end: 4,
                code_len: 1
            }
        )]
    );
}

#[test]
fn empty_and_reversed_function_spans_are_reported() {
    // An empty span has nothing to run: control falls off its end at once.
    assert_eq!(
        rejected(verify_spans(&[Ret, Ret], &[span(0, 1, 1)], false)),
        [(0, 1, DiagnosticKind::MissingTerminator)]
    );
    // A reversed span is invalid even when both ends are inside the code.
    assert_eq!(
        rejected(verify_spans(&[Ret, Ret, Ret], &[span(0, 2, 1)], false)),
        [(
            0,
            2,
            DiagnosticKind::InvalidFunctionSpan {
                start: 2,
                end: 1,
                code_len: 3
            }
        )]
    );
}

#[test]
fn a_jump_with_no_targets_is_a_structural_error() {
    // Without the check, an empty jump would end the path like a terminator.
    assert_eq!(
        rejected(verify(&[Const(Type::I64), Table(vec![])])),
        [(0, 1, DiagnosticKind::EmptyJump)]
    );
    assert_eq!(
        DiagnosticKind::EmptyJump.to_string(),
        "the instruction jumps, but names no target address"
    );
    // A non-empty table is an ordinary jump.
    accepted(verify(&[Table(vec![1, 2]), Ret, Ret]));
}

#[test]
fn duplicate_jump_targets_are_one_edge() {
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 2),      // 1
        Const(Type::I64),  // 2
        Use(Type::I64),    // 3
        Ret,               // 4
    ];
    let verification = accepted(verify_spans(&code, &[span(0, 0, 5)], true));
    let maps = verification.type_maps.unwrap();
    assert_eq!(
        maps.function(0).unwrap().keys().collect::<Vec<_>>(),
        [&0, &2]
    );
    // The type error after the duplicated edge is reported once.
    let mut bad = code.to_vec();
    bad[3] = Use(Type::F64);
    assert_eq!(rejected(verify(&bad)), [(0, 3, mismatch("f64", "i64"))]);
}

#[test]
fn a_local_count_mismatch_at_a_join_is_structured() {
    let mut saved = Frame::new(vec![TypeSlot::Known(Type::I64)]);
    let err = saved.join(&Frame::new(vec![])).unwrap_err();
    assert_eq!(
        err.kind(),
        &DiagnosticKind::LocalCountMismatch {
            expected: 1,
            found: 0
        }
    );
    assert_eq!(
        err.to_string(),
        "local count mismatch at join: expected 1, found 0"
    );
}

#[test]
fn a_type_conflict_at_a_join_is_an_error_only_when_used() {
    assert_eq!(
        rejected(verify(&diamond(Type::I64, Type::F64, Use(Type::I64)))),
        [(0, 5, mismatch("i64", "i64 | f64"))]
    );
    accepted(verify(&diamond(Type::I64, Type::F64, Pop)));
}

#[test]
fn conflicting_locals_join_like_operands() {
    struct Store(u32, Type);
    struct Load(u32, Type);
    enum L {
        Op(Op),
        Store(Store),
        Load(Load),
    }
    impl Verify<FrameState<TypeSlot>> for L {
        fn flow(&self) -> Flow {
            match self {
                L::Op(op) => Verify::<FrameState<TypeSlot>>::flow(op),
                _ => Flow::Next,
            }
        }
        fn verify(&self, cx: &mut FrameState<TypeSlot>) -> eyre::Result<()> {
            match self {
                L::Op(op) => op.verify(cx),
                L::Store(Store(i, ty)) => {
                    let slot = cx.pop_expect(ty)?;
                    Ok(cx.set_local(*i, slot)?)
                }
                L::Load(Load(i, ty)) => {
                    let slot = cx.local(*i)?.clone();
                    slot.expect(ty)?;
                    cx.push(slot);
                    Ok(())
                }
            }
        }
    }
    let code = [
        L::Op(Const(Type::Bool)),
        L::Op(CondBr(2, 5)),
        L::Op(Const(Type::F64)),
        L::Store(Store(0, Type::F64)),
        L::Op(Br(5)),
        L::Load(Load(0, Type::I64)), // 5: join; local 0 is i64 | f64
        L::Op(Ret),
        L::Load(Load(7, Type::I64)), // 7: unreachable, so not verified
    ];
    let entry = Frame::new(vec![TypeSlot::Known(Type::I64)]);
    let function = FunctionSpan {
        function: 0,
        start: 0,
        end: 8,
        entry: entry.clone(),
    };
    let mut cx = FrameState::new(entry);
    let rejection =
        verify_functions(&code, &[function], &mut cx, VerifyOptions::default()).unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 5, mismatch("i64", "i64 | f64")),
            (0, 7, DiagnosticKind::Unreachable),
        ]
    );
}

#[test]
fn a_huge_counted_pop_is_one_underflow() {
    let code = [Const(Type::I64), PopN(1_000_000_000), Ret];
    assert_eq!(
        rejected(verify(&code)),
        [(0, 1, DiagnosticKind::StackUnderflow)]
    );
}

#[test]
fn a_defect_inside_a_loop_is_reported_once() {
    let code = [
        Const(Type::I64),  // 0: loop-carried value
        Const(Type::Bool), // 1: header
        CondBr(3, 10),     // 2
        Const(Type::Bool), // 3
        CondBr(5, 7),      // 4
        Fail("defect"),    // 5: a side exit inside the loop
        Halt,              // 6
        Pop,               // 7: the value becomes an f64, so the header
        Const(Type::F64),  // 8: state changes and the loop body reruns
        Br(1),             // 9
        Ret,               // 10
    ];
    FAIL_RUNS.with(|runs| runs.set(0));
    assert_eq!(
        rejected(verify(&code)),
        [(0, 5, DiagnosticKind::Rule("defect".into()))]
    );
    let runs = FAIL_RUNS.with(Cell::get);
    assert!(
        runs >= 3,
        "the defect ran {runs} times: at least twice converging, once replaying"
    );
}

#[test]
fn a_rule_error_ends_its_block_but_later_blocks_still_report() {
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 5),      // 1
        Add,               // 2: underflow ends this block...
        Use(Type::F64),    // 3: ...so this is not reported
        Halt,              // 4
        Const(Type::I64),  // 5: another block reports independently
        Use(Type::F64),    // 6
        Halt,              // 7
    ];
    assert_eq!(
        rejected(verify(&code)),
        [
            (0, 2, DiagnosticKind::StackUnderflow),
            (0, 6, mismatch("f64", "i64")),
        ]
    );
}

#[test]
fn errors_across_blocks_and_functions_are_sorted() {
    let code = [
        // function 0: 0..5
        Const(Type::Bool), // 0
        CondBr(2, 4),      // 1
        Use(Type::I64),    // 2: underflow
        Ret,               // 3
        Pop,               // 4: underflow, then falls into function 1
        // function 1: 5..9
        Const(Type::F64), // 5
        Use(Type::I64),   // 6: type mismatch
        Ret,              // 7
        Fail("dead"),     // 8: unreachable
    ];
    // Spans are given out of order; diagnostics come back sorted.
    let outcome = verify_spans(&code, &[span(1, 5, 9), span(0, 0, 5)], false);
    let rejection = outcome.unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 2, DiagnosticKind::StackUnderflow),
            (0, 4, DiagnosticKind::StackUnderflow),
            (0, 4, DiagnosticKind::MissingTerminator),
            (1, 6, mismatch("i64", "f64")),
            (1, 8, DiagnosticKind::Unreachable),
        ]
    );
    let report = eyre::Report::from(rejection);
    assert!(
        report
            .to_string()
            .starts_with("verification failed with 4 errors"),
        "{report}"
    );
}

#[test]
fn verify_errors_survive_wrapping_with_context() {
    struct Wrapped;
    impl Verify<FrameState<TypeSlot>> for Wrapped {
        fn verify(&self, cx: &mut FrameState<TypeSlot>) -> eyre::Result<()> {
            use eyre::WrapErr as _;
            cx.pop().wrap_err("while popping the operand")?;
            Ok(())
        }
    }
    let mut cx = FrameState::new(Frame::new(vec![]));
    let rejection = verify_functions(
        &[Wrapped],
        &[span(0, 0, 1)],
        &mut cx,
        VerifyOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 0, DiagnosticKind::StackUnderflow),
            (0, 0, DiagnosticKind::MissingTerminator),
        ]
    );
}

#[test]
fn type_maps_hold_each_reached_leaders_converged_state() {
    let code = diamond(Type::I64, Type::F64, Pop);
    let verification = accepted(verify_spans(&code, &[span(3, 0, 7)], true));
    let maps = verification.type_maps.expect("type maps were requested");
    let operands = |address| maps.get(3, address).map(|f| f.operands().to_vec());
    assert_eq!(operands(0), Some(vec![]));
    assert_eq!(operands(2), Some(vec![]));
    assert_eq!(operands(4), Some(vec![]));
    assert_eq!(
        operands(5),
        Some(vec![TypeSlot::Conflict(Type::I64, Type::F64)])
    );
    assert_eq!(operands(1), None, "not a leader");
    assert_eq!(maps.get(0, 0), None, "no such function");
    assert_eq!(
        maps.iter().map(|(f, a, _)| (f, a)).collect::<Vec<_>>(),
        [(3, 0), (3, 2), (3, 4), (3, 5)]
    );
    assert_eq!(maps.function(3).unwrap().len(), 4);
}

// ---------------------------------------------------------------------------
// Component state: a sequencer whose channels must be opened before use.
// ---------------------------------------------------------------------------

/// A component-specific capability.
trait ChannelState {
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
    fn open(&mut self, channel: u32);
}

/// The composite's instruction set: the fixture dialect plus a sequencer.
enum Seq {
    Core(Op),
    Open(u32),
    Play(u32),
}

impl<C> Verify<C> for Seq
where
    C: OperandStack + ChannelState + ?Sized,
    C::Slot: TypedSlot,
{
    fn flow(&self) -> Flow {
        match self {
            Seq::Core(op) => Verify::<C>::flow(op),
            _ => Flow::Next,
        }
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        match self {
            Seq::Core(op) => op.verify(cx),
            Seq::Open(channel) => {
                cx.open(*channel);
                Ok(())
            }
            Seq::Play(channel) => Ok(cx.require_open(*channel)?),
        }
    }
}

/// A machine-specific context: the standard frame plus the open channels.
struct SeqVerifier {
    frame: FrameState<TypeSlot>,
    open: BTreeSet<u32>,
}

impl OperandStack for SeqVerifier {
    type Slot = TypeSlot;
    fn pop(&mut self) -> Result<TypeSlot, VerifyError> {
        self.frame.pop()
    }
    fn pop_n(&mut self, n: usize) -> Result<Vec<TypeSlot>, VerifyError> {
        self.frame.pop_n(n)
    }
    fn push(&mut self, slot: TypeSlot) {
        self.frame.push(slot);
    }
    fn peek(&self) -> Result<&TypeSlot, VerifyError> {
        self.frame.peek()
    }
    fn depth(&self) -> usize {
        self.frame.depth()
    }
}

impl ChannelState for SeqVerifier {
    fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
        if self.open.contains(&channel) {
            Ok(())
        } else {
            Err(VerifyError::rule(format!("channel {channel} is not open")))
        }
    }
    fn open(&mut self, channel: u32) {
        self.open.insert(channel);
    }
}

impl AbstractState for SeqVerifier {
    type Snapshot = SeqSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        (self.frame.snapshot(), self.open.clone())
    }

    fn restore(&mut self, (frame, open): &Self::Snapshot) {
        self.frame.restore(frame);
        self.open.clone_from(open);
    }

    fn join(
        (frame, open): &mut Self::Snapshot,
        (in_frame, in_open): &Self::Snapshot,
    ) -> Result<bool, VerifyError> {
        // The frame joins first, so a mismatch leaves the saved state whole.
        let frame_changed = frame.join(in_frame)?;
        let before = open.len();
        open.retain(|channel| in_open.contains(channel));
        Ok(frame_changed || open.len() != before)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.frame.set_address(function, address);
    }
}

type SeqSnapshot = (Frame<TypeSlot>, BTreeSet<u32>);

fn verify_seq(code: &[Seq]) -> Result<Verification<SeqSnapshot>, Rejection> {
    let entry = (Frame::new(vec![]), BTreeSet::new());
    let mut cx = SeqVerifier {
        frame: FrameState::new(entry.0.clone()),
        open: BTreeSet::new(),
    };
    let function = FunctionSpan {
        function: 0,
        start: 0,
        end: code.len() as u32,
        entry,
    };
    verify_functions(code, &[function], &mut cx, VerifyOptions::default())
}

#[test]
fn component_state_joins_by_intersection() {
    let program = |else_arm: Seq| {
        [
            Seq::Core(Const(Type::Bool)), // 0
            Seq::Core(CondBr(2, 5)),      // 1
            Seq::Open(3),                 // 2
            Seq::Open(4),                 // 3
            Seq::Core(Br(6)),             // 4
            else_arm,                     // 5
            Seq::Play(3),                 // 6: join
            Seq::Core(Ret),               // 7
        ]
    };
    // Channel 3 is open on only one arm, so not after the join.
    let rejection = verify_seq(&program(Seq::Open(4))).unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [(0, 6, DiagnosticKind::Rule("channel 3 is not open".into()))]
    );
    // Open on both arms: accepted.
    verify_seq(&program(Seq::Open(3))).unwrap();
}
