// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! `ModuleVerifier` against real modules: a test-local dialect whose rules
//! exercise the standard capabilities, and a wrapping machine context.

use std::collections::BTreeSet;

use vihaco_module::module::{
    FieldInfo, FunctionInfo, LocalModule, NamedTypeInfo, NoInfo, Parameter, ProgramMetadata,
    Signature, StructInfo,
};
use vihaco_verifier::{
    AbstractState, ComponentState, Constants, DiagnosticKind, Flow, Frame, Locals, ModuleVerifier,
    NamedTypes, OperandStack, Signatures, StructTypes, Type, TypeSlot, TypedSlot, Value, Verify,
    VerifyError, VerifyOptions, WithState, call_effect, return_effect, verify_module,
};

/// A fixture dialect written only against the standard capabilities.
#[derive(Clone, Debug)]
enum Op {
    /// Push a value of the given type.
    Const(Type),
    /// Push constant `n`, typed from the constant table.
    Constant(u32),
    /// Push local `i`, which must have the given type.
    Load(u32, Type),
    /// Pop a value of the given type into local `i`.
    Store(u32, Type),
    /// Call the function starting at an address, passing `arity` arguments.
    Call { arity: u32, address: u32 },
    /// Return the top `n` operands.
    Ret(u32),
    /// Pop a `struct s` and push the type of its field `slot`.
    GetField(u32, u32),
    /// Pop a `bool`, then branch to either target.
    CondBr(u32, u32),
    /// Unconditional branch.
    Br(u32),
}

use Op::*;

impl<C> Verify<C> for Op
where
    C: Locals + Signatures<Type> + StructTypes<Type> + Constants<Value, Type> + ?Sized,
    C::Slot: TypedSlot,
{
    fn flow(&self) -> Flow {
        match self {
            Ret(_) => Flow::Return,
            CondBr(then, otherwise) => Flow::jump_to([*then, *otherwise]),
            Br(target) => Flow::jump(*target),
            _ => Flow::Next,
        }
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        match self {
            Const(ty) => cx.push(C::Slot::of_type(ty.clone())),
            Constant(index) => {
                let ty = cx.constant_type(*index)?;
                cx.push(C::Slot::of_type(ty));
            }
            Load(index, ty) => {
                let slot = cx.local(*index)?.clone();
                slot.expect(ty)?;
                cx.push(slot);
            }
            Store(index, ty) => {
                let slot = cx.pop_expect(ty)?;
                cx.set_local(*index, slot)?;
            }
            Call { arity, address } => {
                let callee = cx.function_at(*address)?;
                call_effect(cx, callee, *arity as usize)?;
            }
            Ret(count) => return_effect(cx, *count as usize)?,
            GetField(struct_id, slot) => {
                cx.pop_expect(&Type::Struct(*struct_id))?;
                let ty = cx.field_type(*struct_id, *slot)?.clone();
                cx.push(C::Slot::of_type(ty));
            }
            CondBr(..) => {
                cx.pop_expect(&Type::Bool)?;
            }
            Br(_) => {}
        }
        Ok(())
    }
}

/// A function declaration: parameter types, result types, extra locals after
/// the parameters, and its address range.
struct Decl {
    params: Vec<Type>,
    ret: Vec<Type>,
    extra: Vec<Type>,
    span: (u32, u32),
}

fn decl(params: &[Type], ret: &[Type], extra: &[Type], span: (u32, u32)) -> Decl {
    Decl {
        params: params.to_vec(),
        ret: ret.to_vec(),
        extra: extra.to_vec(),
        span,
    }
}

fn function(decl: Decl) -> FunctionInfo<Type> {
    let mut locals = decl.params.clone();
    locals.extend(decl.extra);
    FunctionInfo {
        name: 0,
        signature: Signature {
            params: decl
                .params
                .into_iter()
                .map(|ty| Parameter { name: 0, ty })
                .collect(),
            ret: decl.ret,
        },
        locals,
        start_address: decl.span.0,
        end_address: decl.span.1,
        file: 0,
    }
}

/// A validated module assembled through `ProgramMetadata::into_module`, with
/// a `Point { x: f64, y: i64 }` struct and three constants.
fn module<I>(code: Vec<I>, functions: Vec<Decl>) -> LocalModule<I, Value, Type> {
    ProgramMetadata {
        functions: functions.into_iter().map(function).collect(),
        structs: vec![StructInfo {
            name: 1,
            fields: vec![
                FieldInfo {
                    name: 2,
                    ty: Type::F64,
                },
                FieldInfo {
                    name: 3,
                    ty: Type::I64,
                },
            ],
        }],
        types: vec![
            NamedTypeInfo {
                name: 5,
                repr: Type::U64,
            },
            NamedTypeInfo {
                name: 6,
                repr: Type::Bool,
            },
        ],
        labels: vec![],
        constants: vec![Value::U64(7), Value::F64(1.5), Value::String(4)],
        strings: vec![
            "main".into(),
            "Point".into(),
            "x".into(),
            "y".into(),
            "hello".into(),
            "location".into(),
            "flag".into(),
        ],
        main_function: Some(0),
        source_symbols: vec![],
        extra: NoInfo,
    }
    .into_module(code)
    .unwrap()
}

/// Verify a module, returning its errors as `(function, address, kind)`.
fn errors(module: &LocalModule<Op, Value, Type>) -> Vec<(u32, u32, DiagnosticKind)> {
    match verify_module(module, VerifyOptions::default()) {
        Ok(verification) => {
            assert!(verification.warnings.is_empty(), "{verification:?}");
            vec![]
        }
        Err(rejection) => rejection
            .diagnostics
            .into_iter()
            .map(|d| (d.function, d.address, d.kind))
            .collect(),
    }
}

fn single(
    code: Vec<Op>,
    params: &[Type],
    ret: &[Type],
    extra: &[Type],
) -> Vec<(u32, u32, DiagnosticKind)> {
    let len = code.len() as u32;
    errors(&module(code, vec![decl(params, ret, extra, (0, len))]))
}

fn mismatch(expected: &str, found: &str) -> DiagnosticKind {
    DiagnosticKind::TypeMismatch {
        expected: expected.into(),
        found: found.into(),
    }
}

fn rule(message: &str) -> DiagnosticKind {
    DiagnosticKind::Rule(message.into())
}

#[test]
fn locals_enter_typed_from_their_declarations() {
    let (params, ret, extra) = (&[Type::I64][..], &[Type::I64][..], &[Type::F64][..]);
    assert_eq!(
        single(vec![Load(0, Type::I64), Ret(1)], params, ret, extra),
        []
    );
    assert_eq!(
        single(
            vec![
                Load(1, Type::F64),
                Store(1, Type::F64),
                Load(0, Type::I64),
                Ret(1)
            ],
            params,
            ret,
            extra
        ),
        []
    );
    assert_eq!(
        single(vec![Load(1, Type::I64), Ret(1)], params, ret, extra),
        [(0, 0, mismatch("i64", "f64"))]
    );
    assert_eq!(
        single(vec![Load(2, Type::I64), Ret(1)], params, ret, extra),
        [(
            0,
            0,
            DiagnosticKind::LocalIndexOutOfRange { index: 2, count: 2 }
        )]
    );
    assert_eq!(
        single(
            vec![
                Const(Type::I64),
                Store(9, Type::I64),
                Load(0, Type::I64),
                Ret(1)
            ],
            params,
            ret,
            extra
        ),
        [(
            0,
            1,
            DiagnosticKind::LocalIndexOutOfRange { index: 9, count: 2 }
        )]
    );
}

#[test]
fn stores_retype_locals_and_joins_merge_them() {
    // A store replaces the local's slot; a later typed load sees the new type.
    assert_eq!(
        single(
            vec![
                Const(Type::F64),
                Store(0, Type::F64),
                Load(0, Type::I64),
                Ret(1)
            ],
            &[Type::I64],
            &[Type::I64],
            &[],
        ),
        [(0, 2, mismatch("i64", "f64"))]
    );
    // Retyped on one arm only: the join holds a conflict.
    assert_eq!(
        single(
            vec![
                Const(Type::Bool),
                CondBr(2, 4),
                Const(Type::F64),
                Store(0, Type::F64), // falls through to the join at 4
                Load(0, Type::I64),  // 4: i64 | f64
                Ret(1),
            ],
            &[Type::I64],
            &[Type::I64],
            &[],
        ),
        [(0, 4, mismatch("i64", "i64 | f64"))]
    );
}

#[test]
fn calls_are_checked_against_the_callee_signature() {
    let callee = |main: Vec<Op>| {
        let len = main.len() as u32;
        let mut code = main;
        // The callee `(i64, f64) -> u64`, starting right after main.
        code.extend([
            Load(0, Type::I64),
            Load(1, Type::F64),
            Const(Type::U64),
            Ret(1),
        ]);
        let end = code.len() as u32;
        module(
            code,
            vec![
                decl(&[], &[Type::U64], &[Type::U64], (0, len)),
                decl(&[Type::I64, Type::F64], &[Type::U64], &[], (len, end)),
            ],
        )
    };
    let call = |arity| Call { arity, address: 6 };

    // Arguments popped, results pushed and usable.
    let ok = callee(vec![
        Const(Type::I64),
        Const(Type::F64),
        call(2),
        Store(0, Type::U64),
        Load(0, Type::U64),
        Ret(1),
    ]);
    assert_eq!(errors(&ok), []);

    // The call's operand count must match the declared parameters.
    let arity = callee(vec![
        Const(Type::I64),
        Const(Type::F64),
        call(1),
        Store(0, Type::U64),
        Load(0, Type::U64),
        Ret(1),
    ]);
    assert_eq!(
        errors(&arity),
        [(
            0,
            2,
            DiagnosticKind::CallArityMismatch {
                callee: 1,
                declared: 2,
                found: 1
            }
        )]
    );

    // Arguments are checked bottom first against the parameter types.
    let swapped = callee(vec![
        Const(Type::F64),
        Const(Type::I64),
        call(2),
        Store(0, Type::U64),
        Load(0, Type::U64),
        Ret(1),
    ]);
    assert_eq!(errors(&swapped), [(0, 2, mismatch("i64", "f64"))]);

    // Too few operands is an underflow, not a reach into the locals.
    let short = callee(vec![
        Const(Type::I64),
        Const(Type::I64),
        Store(0, Type::U64),
        call(2),
        Load(0, Type::U64),
        Ret(1),
    ]);
    assert_eq!(errors(&short), [(0, 2, mismatch("u64", "i64"))]);
    let short = callee(vec![
        Const(Type::I64),
        call(2),
        Const(Type::U64),
        Store(0, Type::U64),
        Load(0, Type::U64),
        Ret(1),
    ]);
    assert_eq!(errors(&short), [(0, 1, DiagnosticKind::StackUnderflow)]);

    // A call by address must name a function's first instruction.
    let nowhere = callee(vec![
        Call {
            arity: 0,
            address: 7,
        },
        Const(Type::U64),
        Ret(1),
        Const(Type::U64),
        Ret(1),
        Ret(1),
    ]);
    assert_eq!(
        errors(&nowhere),
        [
            (0, 0, rule("no function starts at address 7")),
            (0, 3, DiagnosticKind::Unreachable),
        ]
    );
}

#[test]
fn returns_are_checked_against_the_current_signature() {
    let ret = &[Type::U64][..];
    assert_eq!(single(vec![Const(Type::U64), Ret(1)], &[], ret, &[]), []);
    // Values below the returned ones are dropped, as at runtime.
    assert_eq!(
        single(
            vec![Const(Type::F64), Const(Type::U64), Ret(1)],
            &[],
            ret,
            &[]
        ),
        []
    );
    assert_eq!(
        single(vec![Const(Type::U64), Ret(0)], &[], ret, &[]),
        [(
            0,
            1,
            DiagnosticKind::ReturnCountMismatch {
                declared: 1,
                found: 0
            }
        )]
    );
    assert_eq!(
        single(
            vec![Const(Type::U64), Const(Type::U64), Ret(2)],
            &[],
            ret,
            &[]
        ),
        [(
            0,
            2,
            DiagnosticKind::ReturnCountMismatch {
                declared: 1,
                found: 2
            }
        )]
    );
    assert_eq!(
        single(vec![Const(Type::I64), Ret(1)], &[], ret, &[]),
        [(0, 1, mismatch("u64", "i64"))]
    );
    assert_eq!(
        single(vec![Ret(1)], &[], ret, &[]),
        [(0, 0, DiagnosticKind::StackUnderflow)]
    );
}

#[test]
fn struct_fields_are_typed_from_the_struct_table() {
    let point = &[Type::Struct(0)][..];
    assert_eq!(
        single(
            vec![Load(0, Type::Struct(0)), GetField(0, 1), Ret(1)],
            point,
            &[Type::I64],
            &[]
        ),
        []
    );
    assert_eq!(
        single(
            vec![Load(0, Type::Struct(0)), GetField(0, 0), Ret(1)],
            point,
            &[Type::I64],
            &[]
        ),
        [(0, 2, mismatch("i64", "f64"))]
    );
    assert_eq!(
        single(
            vec![Load(0, Type::Struct(0)), GetField(0, 2), Ret(1)],
            point,
            &[Type::I64],
            &[]
        ),
        [(
            0,
            1,
            rule("field slot 2 is outside struct 0, which has 2 fields")
        )]
    );
    assert_eq!(
        single(
            vec![Const(Type::I64), GetField(0, 1), Ret(1)],
            point,
            &[Type::I64],
            &[]
        ),
        [(0, 1, mismatch("struct 0", "i64"))]
    );
}

#[test]
fn constants_are_typed_from_the_constant_table() {
    assert_eq!(
        single(vec![Constant(1), Ret(1)], &[], &[Type::F64], &[]),
        []
    );
    assert_eq!(
        single(vec![Constant(2), Ret(1)], &[], &[Type::String], &[]),
        []
    );
    assert_eq!(
        single(vec![Constant(0), Ret(1)], &[], &[Type::F64], &[]),
        [(0, 1, mismatch("f64", "u64"))]
    );
    assert_eq!(
        single(vec![Constant(3), Ret(1)], &[], &[Type::F64], &[]),
        [(
            0,
            0,
            rule("constant 3 is outside the constant table of length 3")
        )]
    );
}

#[test]
fn every_function_is_verified_from_its_own_entry_state() {
    // Table order differs from code order; each function's `ret` is checked
    // against its own signature, and diagnostics sort by function index.
    let code = vec![
        // 0..3: function 1, `(f64) -> f64`
        Load(0, Type::F64),
        Store(1, Type::F64),
        Ret(1), // underflow: the load was stored away
        // 3..7: function 0, `() -> u64`, calls function 2 at 7
        Const(Type::I64),
        Call {
            arity: 1,
            address: 7,
        },
        Ret(1),
        Ret(1), // unreachable
        // 7..9: function 2, `(i64) -> u64`
        Load(0, Type::I64),
        Ret(1), // i64 where u64 is declared
    ];
    let unfixed = module(
        code,
        vec![
            decl(&[], &[Type::U64], &[], (3, 7)),
            decl(&[Type::F64], &[Type::F64], &[Type::F64], (0, 3)),
            decl(&[Type::I64], &[Type::U64], &[], (7, 9)),
        ],
    );
    assert_eq!(
        errors(&unfixed),
        [
            (0, 6, DiagnosticKind::Unreachable),
            (1, 2, DiagnosticKind::StackUnderflow),
            (2, 8, mismatch("u64", "i64")),
        ]
    );

    // Type maps record each function's entry frame at its start address.
    let fixed = module(
        vec![
            // 0..4: function 1
            Load(0, Type::F64),
            Store(1, Type::F64),
            Load(1, Type::F64),
            Ret(1),
            // 4..7: function 0
            Const(Type::I64),
            Call {
                arity: 1,
                address: 7,
            },
            Ret(1),
            // 7..10: function 2
            Load(0, Type::I64),
            Const(Type::U64),
            Ret(1),
        ],
        vec![
            decl(&[], &[Type::U64], &[], (4, 7)),
            decl(&[Type::F64], &[Type::F64], &[Type::F64], (0, 4)),
            decl(&[Type::I64], &[Type::U64], &[], (7, 10)),
        ],
    );
    let verification =
        verify_module(&fixed, VerifyOptions::default().with_type_maps(true)).unwrap();
    assert!(verification.warnings.is_empty());
    let maps = verification.type_maps.unwrap();
    let local = TypeSlot::Known;
    assert_eq!(
        maps.get(1, 0),
        Some(&Frame::new(vec![local(Type::F64), local(Type::F64)]))
    );
    assert_eq!(maps.get(0, 4), Some(&Frame::new(vec![])));
    assert_eq!(maps.get(2, 7), Some(&Frame::new(vec![local(Type::I64)])));
}

#[test]
fn signatures_resolve_addresses_and_the_current_function() {
    let module = module(
        vec![Const(Type::U64), Ret(1), Const(Type::U64), Ret(1)],
        vec![
            decl(&[], &[Type::U64], &[], (0, 2)),
            decl(&[Type::I64], &[Type::U64], &[], (2, 4)),
            decl(&[], &[], &[], (2, 2)), // empty, sharing function 1's start
        ],
    );
    let mut cx = ModuleVerifier::new(&module);
    assert_eq!(cx.function_count(), 3);
    assert_eq!(cx.function_at(0).unwrap(), 0);
    assert_eq!(cx.function_at(2).unwrap(), 1, "the first entry wins");
    assert!(cx.function_at(1).is_err());
    assert!(cx.signature(3).is_err());
    cx.set_address(1, 3);
    assert_eq!(cx.current_function(), 1);
    assert_eq!(cx.current_signature().unwrap().params[0].ty, Type::I64);
    assert_eq!(cx.struct_count(), 1);
    assert_eq!(cx.field_count(0).unwrap(), 2);
    assert!(cx.field_count(1).is_err());
    assert_eq!(cx.constant_count(), 3);
    assert_eq!(cx.constant(0).unwrap(), &Value::U64(7));
    assert_eq!(cx.string_count(), 7);
    assert_eq!(cx.function_spans().len(), 3);
    assert_eq!(
        cx.entry_frame(1),
        Some(Frame::new(vec![TypeSlot::Known(Type::I64)]))
    );
}

#[test]
fn unvalidated_frame_metadata_is_rejected_per_function() {
    // Built as a struct literal, bypassing `ProgramMetadata::validate`.
    let mut module = module(
        vec![Const(Type::U64), Ret(1), Load(0, Type::I64), Ret(1), Ret(0)],
        vec![
            decl(&[], &[Type::U64], &[], (0, 2)),
            decl(&[Type::I64], &[Type::I64], &[], (2, 4)),
            decl(&[Type::U64], &[], &[], (4, 5)),
        ],
    );
    module.functions[1].locals[0] = Type::F64;
    module.functions[2].locals.clear();
    assert_eq!(
        errors(&module),
        [
            (
                1,
                2,
                rule("local 0 is declared f64, but parameter 0 is i64")
            ),
            (2, 4, rule("function declares 0 locals but 1 parameters")),
        ]
    );
    module.functions[2].locals = vec![Type::U64; vihaco_module::module::MAX_LOCALS + 1];
    let errors = errors(&module);
    assert_eq!(errors.len(), 2);
    assert!(matches!(&errors[1].2, DiagnosticKind::Rule(m) if m.contains("more than the maximum")));
}

// ---------------------------------------------------------------------------
// A machine context: the standard capabilities plus component state.

/// A sequencer component's capability: channels must be opened before use.
trait Channels {
    fn open(&mut self, channel: u32);
    fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
}

/// The sequencer's abstract state: the channels open on every path.
#[derive(Clone, Debug, Default, PartialEq)]
struct Open(BTreeSet<u32>);

impl ComponentState for Open {
    fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
        let before = self.0.len();
        self.0.retain(|channel| incoming.0.contains(channel));
        Ok(self.0.len() != before)
    }
}

impl Channels for WithState<'_, Open> {
    fn open(&mut self, channel: u32) {
        self.state_mut().0.insert(channel);
    }

    fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
        if self.state().0.contains(&channel) {
            Ok(())
        } else {
            Err(VerifyError::rule(format!("channel {channel} is not open")))
        }
    }
}

/// The sequencer's instructions, written against its own capability.
#[derive(Clone, Debug)]
enum Seq {
    Open(u32),
    Play(u32),
}

impl<C: Channels + ?Sized> Verify<C> for Seq {
    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        match self {
            Seq::Open(channel) => cx.open(*channel),
            Seq::Play(channel) => cx.require_open(*channel)?,
        }
        Ok(())
    }
}

/// The machine's instruction set: the standard fixture dialect plus the
/// sequencer, as a composite's generated aggregate would be.
#[derive(Clone, Debug)]
enum Machine {
    Op(Op),
    Seq(Seq),
}

impl<C> Verify<C> for Machine
where
    Op: Verify<C>,
    Seq: Verify<C>,
{
    fn flow(&self) -> Flow {
        match self {
            Machine::Op(op) => op.flow(),
            Machine::Seq(seq) => seq.flow(),
        }
    }

    fn verify(&self, cx: &mut C) -> eyre::Result<()> {
        match self {
            Machine::Op(op) => op.verify(cx),
            Machine::Seq(seq) => seq.verify(cx),
        }
    }
}

#[test]
fn a_machine_context_wraps_the_module_verifier_with_component_state() {
    use Machine::{Op as O, Seq as S};
    let verify = |code: Vec<Machine>| {
        let len = code.len() as u32;
        let module = module(
            code,
            vec![decl(&[Type::Bool], &[Type::I64], &[Type::I64], (0, len))],
        );
        let mut cx = ModuleVerifier::new(&module).with_state(Open::default());
        let result = cx.verify(&module.code, VerifyOptions::default().with_type_maps(true));
        result
            .map(|verification| verification.type_maps.unwrap())
            .map_err(|rejection| {
                rejection
                    .diagnostics
                    .into_iter()
                    .map(|d| (d.address, d.kind))
                    .collect::<Vec<_>>()
            })
    };
    let diamond = |else_arm: Machine| {
        vec![
            O(Load(0, Type::Bool)), // 0
            O(CondBr(2, 5)),        // 1
            S(Seq::Open(1)),        // 2
            O(Const(Type::I64)),    // 3
            O(Br(7)),               // 4
            else_arm,               // 5
            O(Const(Type::I64)),    // 6
            S(Seq::Play(1)),        // 7: join
            O(Store(1, Type::I64)), // 8: standard capabilities still apply
            O(Load(1, Type::I64)),  // 9
            O(Ret(1)),              // 10
        ]
    };

    let maps = verify(diamond(S(Seq::Open(1)))).unwrap();
    // The function enters with the given component state and typed locals.
    let (entry, open) = maps.get(0, 0).unwrap();
    assert_eq!(open, &Open::default());
    assert_eq!(entry.locals()[0], TypeSlot::Known(Type::Bool));
    let (frame, open) = maps.get(0, 7).unwrap();
    assert_eq!(open, &Open(BTreeSet::from([1])));
    assert_eq!(frame.depth(), 1);

    assert_eq!(
        verify(diamond(S(Seq::Open(2)))).unwrap_err(),
        [(7, rule("channel 1 is not open"))]
    );
    assert_eq!(
        verify(vec![O(Const(Type::F64)), O(Ret(1))]).unwrap_err(),
        [(1, mismatch("i64", "f64"))]
    );
}

#[test]
fn component_states_combine_as_tuples_and_fail_atomically() {
    #[derive(Clone, Debug, PartialEq)]
    struct Fails;
    impl ComponentState for Fails {
        fn join(&mut self, _: &Self) -> Result<bool, VerifyError> {
            Err(VerifyError::rule("cannot join"))
        }
    }
    let mut saved = (Open(BTreeSet::from([1, 2])), Open(BTreeSet::from([3])));
    let incoming = (Open(BTreeSet::from([1])), Open(BTreeSet::from([3])));
    assert!(saved.join(&incoming).unwrap());
    assert_eq!(saved.0, Open(BTreeSet::from([1])));
    assert!(!saved.join(&incoming).unwrap());

    let mut saved = (Open(BTreeSet::from([1, 2])), Fails);
    let before = saved.clone();
    assert!(saved.join(&(Open(BTreeSet::from([1])), Fails)).is_err());
    assert_eq!(saved, before, "a failed join leaves the saved state");

    // The wrapper's snapshot join is atomic too: a frame depth mismatch
    // leaves the component state unchanged.
    let mut saved = (Frame::<TypeSlot>::new(vec![]), Open(BTreeSet::from([1, 2])));
    let incoming = (
        Frame::with_operands(vec![], vec![TypeSlot::Known(Type::I64)]),
        Open(BTreeSet::new()),
    );
    let before = saved.clone();
    assert!(<WithState<'_, Open> as AbstractState>::join(&mut saved, &incoming).is_err());
    assert_eq!(saved, before);
}

#[test]
fn a_wrapper_can_reach_the_inner_verifier() {
    let module = module(
        vec![Const(Type::U64), Ret(1)],
        vec![decl(&[], &[Type::U64], &[], (0, 2))],
    );
    let mut cx = ModuleVerifier::new(&module).with_state(());
    assert_eq!(cx.verifier().functions().len(), 1);
    cx.verifier_mut()
        .frame_state_mut()
        .push(TypeSlot::Known(Type::Bool));
    assert_eq!(cx.depth(), 1);
    assert_eq!(cx.function_spans()[0].entry, (Frame::new(vec![]), ()));
    assert!(cx.verify(&module.code, VerifyOptions::default()).is_ok());
    let inner = cx.into_verifier();
    assert_eq!(inner.strings().len(), 7);
}

#[test]
fn named_types_resolve_names_and_representations() {
    let module = module(
        vec![Const(Type::U64), Ret(1)],
        vec![decl(&[], &[Type::U64], &[], (0, 2))],
    );
    let cx = ModuleVerifier::new(&module);
    assert_eq!(cx.types().len(), 2);
    assert_eq!(cx.named_type_count(), 2);
    assert_eq!(cx.named_type_index("location").unwrap(), 0);
    assert_eq!(cx.named_type_index("flag").unwrap(), 1);
    assert_eq!(cx.named_type_repr(0).unwrap(), &Type::U64);
    assert_eq!(cx.named_type_repr(1).unwrap(), &Type::Bool);
    assert_eq!(
        cx.named_type_index("Point").unwrap_err().kind(),
        &rule("undefined named type `Point`")
    );
    assert_eq!(
        cx.named_type_repr(2).unwrap_err().kind(),
        &rule("named type 2 is outside the named-type table of length 2")
    );

    // A wrapping context delegates the capability.
    let cx = cx.with_state(());
    assert_eq!(cx.named_type_count(), 2);
    assert_eq!(cx.named_type_index("flag").unwrap(), 1);
    assert_eq!(cx.named_type_repr(0).unwrap(), &Type::U64);
}

#[test]
fn only_verification_or_the_escape_hatch_builds_verified_modules() {
    use vihaco_verifier::Verified;

    let main = || vec![decl(&[], &[Type::U64], &[], (0, 2))];
    let good = || module(vec![Const(Type::U64), Ret(1)], main());
    let verified = Verified::new(good()).unwrap();
    assert_eq!(verified.code.len(), 2);
    assert_eq!(verified.get().functions.len(), 1);

    let options = VerifyOptions::default().with_type_maps(true);
    let (verified, verification) = Verified::verify(good(), options).unwrap();
    assert!(verification.warnings.is_empty());
    let maps = verification.type_maps.unwrap();
    assert_eq!(maps.get(0, 0), Some(&Frame::new(vec![])));
    assert_eq!(verified.into_inner().code.len(), 2);

    let bad = module(vec![Const(Type::F64), Ret(1)], main());
    let rejection = Verified::verify(bad.clone(), options).unwrap_err();
    assert_eq!(rejection.errors().count(), 1);
    assert!(
        rejection.type_maps.is_some(),
        "type maps come with rejections"
    );
    // Rejections convert into eyre reports.
    let report: eyre::Report = Verified::new(bad.clone()).unwrap_err().into();
    assert!(
        report
            .to_string()
            .starts_with("verification failed with 1 error"),
        "{report}"
    );

    // The escape hatch skips verification entirely.
    assert_eq!(Verified::assume_verified(bad).code.len(), 2);

    // Component state takes part through `verify_with_state`.
    let (verified, _) = Verified::verify_with_state(good(), (), VerifyOptions::default()).unwrap();
    assert_eq!(verified.code.len(), 2);
}
