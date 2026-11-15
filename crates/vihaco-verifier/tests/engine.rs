// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The verification engine against small hand-written fixture dialects.

use std::cell::Cell;
use std::collections::BTreeSet;

use vihaco_verifier::{
    AbstractState, Diagnostic, DiagnosticKind, Flow, Frame, FrameState, FunctionSpan, JoinPoint,
    Locals, OperandStack, Predecessor, Rejection, Severity, Type, TypeSlot, TypedSlot,
    Verification, Verify, VerifyError, VerifyOptions, stack_effect, verify_functions,
};

thread_local! {
    /// How many times a `Fail` rule has run, to observe convergence.
    static FAIL_RUNS: Cell<usize> = const { Cell::new(0) };
    /// How many times any rule has run, to bound the engine's total work.
    static RULE_RUNS: Cell<usize> = const { Cell::new(0) };
    /// How many times a `Mark` has run: placed at a function's entry, it
    /// counts the whole-function fixpoints the engine runs.
    static MARK_RUNS: Cell<usize> = const { Cell::new(0) };
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
    /// Pop a `u64` count, then that many values: the count is a runtime
    /// value, so the rule reports the instruction as unverifiable.
    PopDynamic,
    /// A no-op that counts its runs.
    Mark,
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
        RULE_RUNS.with(|runs| runs.set(runs.get() + 1));
        match self {
            Op::Const(ty) => cx.push(C::Slot::of_type(ty.clone())),
            Op::Pop => {
                cx.pop()?;
            }
            Op::PopN(n) => {
                cx.pop_n(*n)?;
            }
            Op::Use(ty) => {
                cx.pop_typed(ty)?;
            }
            Op::Add => stack_effect(cx, [Type::I64, Type::I64], [Type::I64])?,
            Op::CondBr(..) => {
                cx.pop_typed(&Type::Bool)?;
            }
            Op::Br(_) | Op::Ret | Op::Halt | Op::Table(_) => {}
            Op::Mark => MARK_RUNS.with(|runs| runs.set(runs.get() + 1)),
            Op::Fail(message) => {
                FAIL_RUNS.with(|runs| runs.set(runs.get() + 1));
                eyre::bail!("{message}");
            }
            Op::PopDynamic => {
                cx.pop_typed(&Type::U64)?;
                return Err(VerifyError::unverifiable(
                    "pops a number of operands given by a runtime value",
                )
                .into());
            }
        }
        Ok(())
    }
}

use Op::*;

type Outcome = Result<Verification<Frame<TypeSlot>>, Rejection<Frame<TypeSlot>>>;

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

/// A type mismatch with no role, as `pop_typed` reports it.
fn mismatch(expected: Type, found: Type) -> DiagnosticKind {
    VerifyError::type_mismatch(expected, TypeSlot::Known(found)).into_kind()
}

/// A mismatch whose value is a conflict between `a` and `b` at join `join`.
fn conflict(expected: Type, a: Type, b: Type, join: u32) -> DiagnosticKind {
    let found = TypeSlot::Conflict {
        first: a,
        second: b,
        join: Some(join),
    };
    VerifyError::type_mismatch(expected, found).into_kind()
}

fn underflow(needed: usize, depth: usize) -> DiagnosticKind {
    DiagnosticKind::StackUnderflow { needed, depth }
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
    // The branch at 4 reaches the join first; falling through from 5 second.
    let Err(rejection) = verify(&code) else {
        panic!("expected a rejection")
    };
    assert_eq!(
        found(&rejection.diagnostics),
        [(
            0,
            6,
            DiagnosticKind::StackDepthMismatch {
                expected: 2,
                found: 1,
                expected_from: Predecessor::Jump(4),
                found_from: Predecessor::Fallthrough(5),
            }
        )]
    );
    assert_eq!(
        rejection.diagnostics[0].to_string(),
        "error: operand stack depth differs where paths join: 2 from address 4 (a jump), 1 from \
         address 5 (falling through)\n  --> function 0, address 6"
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
                found: 2,
                expected_from: Predecessor::Fallthrough(0),
                found_from: Predecessor::Jump(4),
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
            (
                0,
                1,
                DiagnosticKind::InvalidControlFlowTarget {
                    target: 3,
                    start: 0,
                    end: 3
                }
            ),
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
    let rejection = verify(&[Const(Type::I64), Table(vec![])]).unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [(0, 1, DiagnosticKind::EmptyJump)]
    );
    assert_eq!(
        rejection.diagnostics[0].to_string(),
        "error: the instruction jumps, but names no target address (use a return or halt to \
         end a path)\n  --> function 0, address 1"
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
    assert_eq!(
        rejected(verify(&bad)),
        [(0, 3, mismatch(Type::F64, Type::I64))]
    );
}

#[test]
fn a_local_count_mismatch_at_a_join_is_structured() {
    let mut saved = Frame::new(vec![TypeSlot::Known(Type::I64)]);
    let at = JoinPoint {
        function: 0,
        address: 4,
        saved_from: Predecessor::Jump(1),
        incoming_from: Predecessor::Fallthrough(3),
    };
    let err = saved.join(&Frame::new(vec![]), &at).unwrap_err();
    assert_eq!(
        err.kind(),
        &DiagnosticKind::LocalCountMismatch {
            expected: 1,
            found: 0
        }
    );
    assert_eq!(
        err.to_string(),
        "local count differs where paths join: 1 saved, 0 arriving"
    );
}

#[test]
fn a_type_conflict_at_a_join_is_an_error_only_when_used() {
    assert_eq!(
        rejected(verify(&diamond(Type::I64, Type::F64, Use(Type::I64)))),
        [(0, 5, conflict(Type::I64, Type::I64, Type::F64, 5))]
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
                    let slot = cx.pop_typed(ty)?;
                    Ok(cx.set_local(*i, slot)?)
                }
                L::Load(Load(i, ty)) => {
                    let slot = cx.local(*i)?.clone();
                    slot.check(ty)?;
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
            (0, 5, conflict(Type::I64, Type::I64, Type::F64, 5)),
            (0, 7, DiagnosticKind::Unreachable),
        ]
    );
}

#[test]
fn a_huge_counted_pop_is_one_underflow() {
    let code = [Const(Type::I64), PopN(1_000_000_000), Ret];
    assert_eq!(
        rejected(verify(&code)),
        [(0, 1, underflow(1_000_000_000, 1))]
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

/// `Add`'s first operand is an `i64`/`f64` conflict from the join at `join`.
fn add_operand_conflict(join: u32) -> DiagnosticKind {
    let found = TypeSlot::Conflict {
        first: Type::I64,
        second: Type::F64,
        join: Some(join),
    };
    VerifyError::type_mismatch(Type::I64, found)
        .with_role(vihaco_verifier::Role::Operand(0))
        .into_kind()
}

/// A loop whose header fails once the back edge brings an `f64` in. Its exit
/// at 8 is reachable only through the header, so it is not analysed: in the
/// first round the header passed on an `i64`, but at convergence it passes
/// nothing on, and an error at 8 from that early state would be a follow-on.
fn a_loop_that_fails_at_its_header() -> Vec<Op> {
    vec![
        Const(Type::I64),  // 0
        Const(Type::I64),  // 1: loop header
        Add,               // 2: fails once the back edge brings an f64
        Const(Type::Bool), // 3
        CondBr(5, 8),      // 4
        Pop,               // 5
        Const(Type::F64),  // 6
        Br(1),             // 7
        Use(Type::F64),    // 8: reachable only through the header
        Ret,               // 9
    ]
}

#[test]
fn code_reachable_only_through_a_failing_block_is_not_analysed() {
    let mut cx = FrameState::new(Frame::new(vec![]));
    let options = VerifyOptions::default().with_type_maps(true);
    let code = a_loop_that_fails_at_its_header();
    let rejection = verify_functions(&code, &[span(0, 0, 10)], &mut cx, options).unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [(0, 2, add_operand_conflict(1))]
    );
    let maps = rejection.type_maps.expect("type maps were requested");
    let operands = |address| maps.get(0, address).map(|f| f.operands().to_vec());
    // The header is mapped at the state it fails at: the join of the entry
    // path and the back edge.
    assert_eq!(
        operands(1),
        Some(vec![TypeSlot::Conflict {
            first: Type::I64,
            second: Type::F64,
            join: Some(1),
        }])
    );
    // The loop body and the exit are reached only through the header.
    assert_eq!(operands(5), None);
    assert_eq!(operands(8), None);
}

#[test]
fn a_block_failing_only_on_a_follow_on_state_is_analysed_from_its_other_paths() {
    // The loop above, moved to 2..=11, with its exit at 10 also reached from
    // 12 with an f64. In the first round the header passed an i64 to 10, so
    // 10's full fixpoint state is a conflict and it fails; from the path
    // through 12 alone, the one left at convergence, it passes.
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 12),     // 1
        Const(Type::I64),  // 2
        Const(Type::I64),  // 3: loop header
        Add,               // 4: fails once the back edge brings an f64
        Const(Type::Bool), // 5
        CondBr(7, 10),     // 6
        Pop,               // 7
        Const(Type::F64),  // 8
        Br(3),             // 9
        Use(Type::F64),    // 10
        Ret,               // 11
        Const(Type::F64),  // 12
        Br(10),            // 13
    ];
    assert_eq!(rejected(verify(&code)), [(0, 4, add_operand_conflict(3))]);
}

/// The follow-on case above, except that the other path into 10 brings two
/// `bool`s, so 10 fails for real. The header at 3 fails, so the edge 6 -> 10
/// exists only through a failing block.
fn a_real_defect_after_a_failing_loop() -> Vec<Op> {
    vec![
        Const(Type::Bool), // 0
        CondBr(2, 12),     // 1
        Const(Type::I64),  // 2
        Const(Type::I64),  // 3: loop header
        Add,               // 4: fails once the back edge brings an f64
        Const(Type::Bool), // 5
        CondBr(7, 10),     // 6
        Pop,               // 7
        Const(Type::F64),  // 8
        Br(3),             // 9
        Use(Type::F64),    // 10: fails for real on the path through 12
        Ret,               // 11
        Const(Type::Bool), // 12
        Const(Type::Bool), // 13
        Br(10),            // 14
    ]
}

#[test]
fn a_failing_block_reports_its_real_defect_not_a_follow_on() {
    // In the first round the header passed an i64 to 10. Neither that
    // state, nor a depth mismatch between it and the path through 14, may be
    // reported: 10 is analysed from the path through 14 alone.
    let rejection = verify_spans(
        &a_real_defect_after_a_failing_loop(),
        &[span(0, 0, 15)],
        true,
    )
    .unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 4, add_operand_conflict(3)),
            (0, 10, mismatch(Type::F64, Type::Bool)),
        ]
    );
    let maps = rejection.type_maps.expect("type maps were requested");
    let operands = |address| maps.get(0, address).map(|f| f.operands().to_vec());
    // 10 is mapped at the state it fails at, from the path through 14 alone.
    let bool_slot = TypeSlot::Known(Type::Bool);
    assert_eq!(operands(10), Some(vec![bool_slot.clone(), bool_slot]));
    // The loop body is reached only through the failing header.
    assert_eq!(operands(7), None);
}

#[test]
fn a_join_into_a_failing_block_is_checked_only_between_analysed_edges() {
    // In the first round the header passed an i64 on to 10, and so through
    // 12 to 16, which then fails. At convergence only the path through 13
    // reaches 10, so 12 passes two bools to 16: checking that edge against
    // 16's state from the first round would report a depth mismatch between
    // the edge from 12 and itself.
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 13),     // 1
        Const(Type::I64),  // 2
        Const(Type::I64),  // 3: loop header
        Add,               // 4: fails once the back edge brings an f64
        Const(Type::Bool), // 5
        CondBr(7, 10),     // 6
        Pop,               // 7
        Const(Type::F64),  // 8
        Br(3),             // 9
        Const(Type::Bool), // 10
        Pop,               // 11
        Br(16),            // 12
        Const(Type::Bool), // 13
        Const(Type::Bool), // 14
        Br(10),            // 15
        Use(Type::F64),    // 16
        Ret,               // 17
    ];
    let rejection = verify_spans(&code, &[span(0, 0, 18)], true).unwrap_err();
    assert_eq!(
        found(&rejection.diagnostics),
        [
            (0, 4, add_operand_conflict(3)),
            (0, 16, mismatch(Type::F64, Type::Bool)),
        ]
    );
    let maps = rejection.type_maps.expect("type maps were requested");
    let operands = |address| maps.get(0, address).map(|f| f.operands().to_vec());
    let bool_slot = TypeSlot::Known(Type::Bool);
    assert_eq!(operands(16), Some(vec![bool_slot.clone(), bool_slot]));
}

/// Verify `code`, which must be rejected with `errors` errors, and assert
/// that the engine ran at most `per_instruction` rules per instruction.
fn assert_rejected_in_linear_time(code: &[Op], errors: usize, per_instruction: usize) {
    RULE_RUNS.with(|runs| runs.set(0));
    let rejection = verify(code).unwrap_err();
    assert_eq!(rejection.errors().count(), errors);
    let runs = RULE_RUNS.with(Cell::get);
    assert!(
        runs <= per_instruction * code.len(),
        "rules ran {runs} times for {} instructions and {errors} failing blocks",
        code.len()
    );
}

#[test]
fn many_independent_failing_blocks_settle_in_linear_time() {
    // Each copy of `if cond { fail; return }` fails on its own, and no failing
    // block can reach another, so no restore can succeed: settling must not
    // rerun the whole-function fixpoint once per failing block.
    let copies = 2_000u32;
    let mut code = Vec::new();
    for copy in 0..copies {
        let base = 4 * copy;
        code.extend([
            Const(Type::Bool),
            CondBr(base + 2, base + 4),
            Fail("independent"),
            Ret,
        ]);
    }
    code.push(Ret);
    assert_rejected_in_linear_time(&code, copies as usize, 8);
}

#[test]
fn many_failing_arms_that_rejoin_settle_in_linear_time() {
    // `if cond { fail }; if cond { fail }; ...`: every failing arm rejoins the
    // code after it, so each reaches every later arm in the control-flow
    // graph. But no arm ever passes a state on, so cutting one changes
    // nothing downstream, and settling must still not rerun the
    // whole-function fixpoint once per failing arm.
    let copies = 2_000u32;
    let mut code = Vec::new();
    for copy in 0..copies {
        let base = 4 * copy;
        code.extend([
            Const(Type::Bool),
            CondBr(base + 2, base + 4),
            Fail("rejoining"),
            Br(base + 4),
        ]);
    }
    code.push(Ret);
    assert_rejected_in_linear_time(&code, copies as usize, 8);
}

/// Verify `code`, whose entry block starts with a `Mark` and has no incoming
/// edge, and return the rejection's diagnostics and how many times the entry
/// block ran. It runs once per whole-function fixpoint, at most once more per
/// fixpoint scanning for failing blocks, and once replaying, so a function
/// that settles in `n` fixpoints besides the uncut one runs it at most
/// `2 * n + 3` times.
fn rejected_counting_fixpoints(code: &[Op]) -> (Vec<(u32, u32, DiagnosticKind)>, usize) {
    MARK_RUNS.with(|runs| runs.set(0));
    let found = rejected(verify(code));
    (found, MARK_RUNS.with(Cell::get))
}

/// The `i64`/`bool` conflict from the join at `join` that an `i64` rule
/// finds, as `Add`'s first operand (`add`) or as a plain pop.
fn bool_conflict(join: u32, add: bool) -> DiagnosticKind {
    let found = TypeSlot::Conflict {
        first: Type::I64,
        second: Type::Bool,
        join: Some(join),
    };
    let error = VerifyError::type_mismatch(Type::I64, found);
    if add {
        error
            .with_role(vihaco_verifier::Role::Operand(0))
            .into_kind()
    } else {
        error.into_kind()
    }
}

/// One copy of `if cond { x = 1; loop { x = x + 1; x = true } }`, at `base`:
/// the loop header at `base + 3` passes on first entry and fails once the
/// back edge brings a `bool`. The `if` skips to `skip`.
fn failing_loop(base: u32, skip: u32) -> [Op; 10] {
    [
        Const(Type::Bool),          // base
        CondBr(base + 2, skip),     // base + 1
        Const(Type::I64),           // base + 2
        Const(Type::I64),           // base + 3: loop header
        Add,                        // base + 4: fails on `i64 | bool`
        Pop,                        // base + 5
        Const(Type::Bool),          // base + 6
        Const(Type::Bool),          // base + 7
        CondBr(base + 3, base + 9), // base + 8: back edge, or exit
        Pop,                        // base + 9
    ]
}

#[test]
fn failing_loops_cut_off_by_an_earlier_one_cost_no_fixpoint_each() {
    // Both arms of each `if` enter the loop, so the first loop header, once
    // cut, cuts off every later copy: they have no state, and need no
    // fixpoint of their own even though the first header reaches them.
    let copies = 2_000u32;
    let mut code = vec![Mark];
    for copy in 0..copies {
        let base = 1 + 10 * copy;
        code.extend(failing_loop(base, base + 2));
    }
    code.push(Ret);
    let (found, runs) = rejected_counting_fixpoints(&code);
    assert_eq!(found, [(0, 5, bool_conflict(4, true))]);
    assert!(runs <= 10, "the entry block ran {runs} times");
}

#[test]
fn many_bypassed_failing_loops_settle_within_the_budget() {
    // Each loop sits behind an `if`, so every header is reached and fails,
    // and each one passed a state on in the uncut fixpoint, so each later
    // header is reached by an emitting cut block: settling caps the
    // fixpoints it spends on them, and still reports every header.
    let copies = 2_000u32;
    let mut code = vec![Mark];
    for copy in 0..copies {
        let base = 1 + 10 * copy;
        code.extend(failing_loop(base, base + 10));
    }
    code.push(Ret);
    RULE_RUNS.with(|runs| runs.set(0));
    let (found, runs) = rejected_counting_fixpoints(&code);
    let expected: Vec<_> = (0..copies)
        .map(|copy| (0, 1 + 10 * copy + 4, bool_conflict(1 + 10 * copy + 3, true)))
        .collect();
    assert_eq!(found, expected);
    assert!(runs <= 2 * 65 + 3, "the entry block ran {runs} times");
    let rules = RULE_RUNS.with(Cell::get);
    assert!(rules <= 100 * code.len(), "rules ran {rules} times");
}

/// One copy of `x = 1; if cond { loop { ... } }; use(x)`, at `base`: the loop
/// exit and the bypass join at `base + 9`, which fails on `i64 | bool` only
/// because the failing loop header at `base + 3` passed a `bool` on in an
/// early round. It is a follow-on failure that settling restores.
fn failing_loop_joining_a_bypass(base: u32) -> [Op; 10] {
    [
        Const(Type::I64),           // base
        Const(Type::Bool),          // base + 1
        CondBr(base + 3, base + 9), // base + 2
        Const(Type::I64),           // base + 3: loop header
        Add,                        // base + 4: fails on `i64 | bool`
        Pop,                        // base + 5
        Const(Type::Bool),          // base + 6
        Const(Type::Bool),          // base + 7
        CondBr(base + 3, base + 9), // base + 8: back edge, or exit
        Use(Type::I64),             // base + 9: the join
    ]
}

fn failing_loops_joining_bypasses(copies: u32) -> Vec<Op> {
    let mut code = vec![Mark];
    for copy in 0..copies {
        code.extend(failing_loop_joining_a_bypass(1 + 10 * copy));
    }
    code.push(Ret);
    code
}

#[test]
fn follow_on_failures_are_restored_within_the_budget() {
    let (found, _) = rejected_counting_fixpoints(&failing_loops_joining_bypasses(5));
    let expected: Vec<_> = (0..5)
        .map(|copy| (0, 1 + 10 * copy + 4, bool_conflict(1 + 10 * copy + 3, true)))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn past_the_budget_the_real_defects_are_still_reported() {
    // Each join is restored only after fixpoints for its loop header and for
    // itself, so 2,000 copies exhaust the budget. Settling then stops: the
    // join of the last copy it reached stays cut, reports a follow-on error,
    // and cuts off the copies after it. The loop headers before it, the real
    // defects, are still reported, and the work stays bounded.
    let copies = 2_000u32;
    let code = failing_loops_joining_bypasses(copies);
    RULE_RUNS.with(|runs| runs.set(0));
    let (found, runs) = rejected_counting_fixpoints(&code);
    let headers = found.len() as u32 - 1;
    assert!(
        (1..copies).contains(&headers),
        "the budget was not exhausted: {found:?}"
    );
    let mut expected: Vec<_> = (0..headers)
        .map(|copy| (0, 1 + 10 * copy + 4, bool_conflict(1 + 10 * copy + 3, true)))
        .collect();
    let last_join = 1 + 10 * (headers - 1) + 9;
    expected.push((0, last_join, bool_conflict(last_join, false)));
    assert_eq!(found, expected);
    assert!(runs <= 2 * 65 + 3, "the entry block ran {runs} times");
    let rules = RULE_RUNS.with(Cell::get);
    assert!(rules <= 200 * code.len(), "rules ran {rules} times");
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
            (0, 2, underflow(2, 0)),
            (0, 6, mismatch(Type::F64, Type::I64)),
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
            (0, 2, underflow(1, 0)),
            (0, 4, underflow(1, 0)),
            (0, 4, DiagnosticKind::MissingTerminator),
            (1, 6, mismatch(Type::I64, Type::F64)),
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
            (0, 0, underflow(1, 0)),
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
        Some(vec![TypeSlot::Conflict {
            first: Type::I64,
            second: Type::F64,
            join: Some(5)
        }])
    );
    assert_eq!(operands(1), None, "not a leader");
    assert_eq!(maps.get(0, 0), None, "no such function");
    assert_eq!(
        maps.iter().map(|(f, a, _)| (f, a)).collect::<Vec<_>>(),
        [(3, 0), (3, 2), (3, 4), (3, 5)]
    );
    assert_eq!(maps.function(3).unwrap().len(), 4);
}

#[test]
fn any_reached_address_is_one_replay_from_its_leader() {
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 4),      // 1
        Const(Type::I64),  // 2
        Br(6),             // 3
        Const(Type::F64),  // 4
        Const(Type::U64),  // 5: falls into the join
        Pop,               // 6: join at depth 1 or 2, rejected...
        Ret,               // 7
        Ret,               // 8: unreachable
    ];
    // ...so use a well-typed variant first.
    let mut good = code.clone();
    good[5] = Br(6);
    let maps = accepted(verify_spans(&good, &[span(0, 0, 9)], true))
        .type_maps
        .unwrap();
    let mut cx = FrameState::new(Frame::new(vec![]));
    let operands = |cx: &mut FrameState<TypeSlot>, address| {
        maps.state_at(&good, cx, 0, address)
            .map(|frame| frame.operands().to_vec())
    };
    let known = TypeSlot::Known;
    // Not a leader: replayed from the leader at 0.
    assert_eq!(operands(&mut cx, 1), Some(vec![known(Type::Bool)]));
    // Leaders return their converged state.
    assert_eq!(operands(&mut cx, 2), Some(vec![]));
    assert_eq!(operands(&mut cx, 3), Some(vec![known(Type::I64)]));
    assert_eq!(
        operands(&mut cx, 7),
        Some(vec![]),
        "after the join's pop of the conflicted slot"
    );
    assert_eq!(operands(&mut cx, 8), None, "unreachable");
    assert_eq!(operands(&mut cx, 9), None, "outside the function");
    assert_eq!(
        maps.state_at(&good, &mut cx, 1, 0),
        None,
        "no such function"
    );

    // In a rejection the state first saved at a mismatched join stands, so
    // the replay continues from it.
    let Err(rejection) = verify_spans(&code, &[span(0, 0, 9)], true) else {
        panic!("expected a rejection");
    };
    let maps = rejection.type_maps.unwrap();
    assert_eq!(
        maps.state_at(&code, &mut cx, 0, 5).map(|f| f.depth()),
        Some(1)
    );
    assert_eq!(
        maps.state_at(&code, &mut cx, 0, 7).map(|f| f.depth()),
        Some(0)
    );
}

#[test]
fn type_maps_are_returned_with_a_rejection() {
    // The join fails, so code after it is never reached by a successful
    // block, but the leaders before it still have states.
    let code = [
        Const(Type::Bool), // 0
        CondBr(2, 4),      // 1
        Const(Type::I64),  // 2
        Br(5),             // 3
        Const(Type::F64),  // 4
        Use(Type::I64),    // 5 (join): conflict
        Br(7),             // 6
        Ret,               // 7: only reachable through the failed block
    ];
    let Err(rejection) = verify_spans(&code, &[span(0, 0, 8)], true) else {
        panic!("expected a rejection");
    };
    assert_eq!(rejection.errors().count(), 1);
    assert_eq!(rejection.diagnostics[0].address, 5);
    let maps = rejection.type_maps.expect("type maps were requested");
    let operands = |address| maps.get(0, address).map(|f| f.operands().to_vec());
    assert_eq!(operands(0), Some(vec![]));
    assert_eq!(
        operands(5),
        Some(vec![TypeSlot::Conflict {
            first: Type::I64,
            second: Type::F64,
            join: Some(5)
        }])
    );
    assert_eq!(operands(7), None, "reached only through the failed block");

    // Without the option, a rejection carries no type maps either.
    let Err(rejection) = verify_spans(&code, &[span(0, 0, 8)], false) else {
        panic!("expected a rejection");
    };
    assert!(rejection.type_maps.is_none());
}

#[test]
fn a_runtime_operand_count_is_unverifiable() {
    let errors = rejected(verify(&[Const(Type::U64), PopDynamic, Ret]));
    assert_eq!(
        errors,
        [(
            0,
            1,
            DiagnosticKind::Unverifiable {
                reason: "pops a number of operands given by a runtime value".into()
            }
        )]
    );
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
        at: &JoinPoint,
    ) -> Result<bool, VerifyError> {
        // The frame joins first, so a mismatch leaves the saved state whole.
        let frame_changed = frame.join(in_frame, at)?;
        let before = open.len();
        open.retain(|channel| in_open.contains(channel));
        Ok(frame_changed || open.len() != before)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.frame.set_address(function, address);
    }
}

type SeqSnapshot = (Frame<TypeSlot>, BTreeSet<u32>);

fn verify_seq(code: &[Seq]) -> Result<Verification<SeqSnapshot>, Rejection<SeqSnapshot>> {
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
