// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! End-to-end verification: real SST programs are parsed and lowered through
//! the metadata loading pipeline into modules, then verified with
//! `verify_module` against a real composite's generated `Verify` aggregate.
//!
//! The same programs run against both typed builds. `P::` and `C::` stand for
//! the device routes of program and computation instructions: with
//! `program-image-execution` those are the stdlib program device and the CPU;
//! with `typed-declarations` alone the CPU owns both.

#![cfg(feature = "typed-declarations")]

use vihaco_abi::{Type, Value, traits::FromText};
use vihaco_bytecode::{NoContext, SstFile, SstHeader};
use vihaco_module::{
    loader::MetadataLoweringContext,
    module::{LocalModule, ProgramMetadata},
};
use vihaco_runtime::HasInstructionSet;
use vihaco_verifier::{
    Diagnostic, DiagnosticKind, Expected, TypeSlot, VerifyError, VerifyOptions, verify_module,
};

#[cfg(feature = "program-image-execution")]
mod machine {
    use vihaco_cpu::CPU;
    use vihaco_runtime::{Apply, BuildMessage, Execute, Step, composite, field};
    use vihaco_stdlib::ExecutionState;

    pub const P: &str = "execution::program::";
    pub const C: &str = "cpu::cpu::";

    #[composite]
    pub struct Machine {
        #[device(0)]
        #[program]
        pub execution: vihaco_stdlib::ExecutionState,
        #[device(1)]
        pub cpu: vihaco_cpu::CPU,
    }

    type Module = super::Module;

    impl<I> Step<field!(Machine::execution), I, Module> for Machine
    where
        ExecutionState: Execute<I, Effect = (), Error = eyre::Report>,
        Self: BuildMessage<field!(Machine::execution), I, Module>,
    {
        fn step(&mut self, instruction: &I, module: &Module) -> eyre::Result<()> {
            let message =
                <Self as BuildMessage<field!(Machine::execution), I, Module>>::build_message(
                    self,
                    instruction,
                    module,
                )?;
            self.execution.execute(instruction, message)
        }
    }

    impl<I> Step<field!(Machine::cpu), I, Module> for Machine
    where
        CPU: Execute<I, Error = eyre::Report>,
        Self: BuildMessage<field!(Machine::cpu), I, Module>,
        ExecutionState: Apply<<CPU as Execute<I>>::Effect>,
    {
        fn step(&mut self, instruction: &I, module: &Module) -> eyre::Result<()> {
            let message = <Self as BuildMessage<field!(Machine::cpu), I, Module>>::build_message(
                self,
                instruction,
                module,
            )?;
            let effect = self.cpu.execute(instruction, message)?;
            self.execution.apply(effect);
            Ok(())
        }
    }
}

#[cfg(not(feature = "program-image-execution"))]
mod machine {
    use vihaco_runtime::composite;

    pub const P: &str = "cpu::cpu::";
    pub const C: &str = "cpu::cpu::";

    #[composite]
    pub struct Machine {
        #[device(0)]
        pub cpu: vihaco_cpu::CPU,
    }
}

use machine::Machine;

type Module = LocalModule<<Machine as HasInstructionSet>::Runtime, Value, Type>;

/// A header holding only program metadata, with types written as `Ty`.
struct Header<Ty = Type>(ProgramMetadata<Value, Ty>);

impl<Ty> FromText for Header<Ty>
where
    ProgramMetadata<Value, Ty>: FromText,
{
    fn from_text(text: &str) -> eyre::Result<Self> {
        ProgramMetadata::from_text(text).map(Self)
    }
}

impl<Ty> SstHeader for Header<Ty> where ProgramMetadata<Value, Ty>: FromText {}

/// One function of a test program. Parameters are named `p0`, `p1`, …;
/// `locals` lists the locals declared after the parameters.
struct Function<'a> {
    name: &'a str,
    params: &'a [&'a str],
    ret: &'a [&'a str],
    locals: &'a [&'a str],
    body: &'a [&'a str],
}

/// The string table every test program shares: the struct and named-type
/// tables below refer to it, and function names follow it.
const STRINGS: [&str; 12] = [
    "test.sst", "Point", "x", "y", "location", "lane", "Pair", "a", "p0", "p1", "p2", "p3",
];

/// `Point { x: i64, y: f64 }` and `Pair { a: u64 }`.
const STRUCTS: &str = "[
        { name: 1, fields: [{ name: 2, ty: i64 }, { name: 3, ty: f64 }] },
        { name: 6, fields: [{ name: 7, ty: u64 }] },
    ]";

/// `location` and `lane`, both represented by `u64`.
const TYPES: &str = "[{ name: 4, repr: u64 }, { name: 5, repr: u64 }]";

/// Render a complete SST file for `functions`, with metadata addresses
/// computed from the bodies, and with `P::` / `C::` replaced by the build's
/// device routes.
fn sst(functions: &[Function]) -> String {
    let mut strings: Vec<String> = STRINGS.iter().map(|s| format!("{s:?}")).collect();
    let mut metadata = Vec::new();
    let mut text = String::new();
    let mut address = 0;
    for function in functions {
        let name = strings.len();
        strings.push(format!("{:?}", function.name));
        let params: Vec<String> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("{{ name: {}, ty: {ty} }}", 8 + index))
            .collect();
        let locals: Vec<&str> = function
            .params
            .iter()
            .chain(function.locals)
            .copied()
            .collect();
        let end = address + function.body.len();
        metadata.push(format!(
            "{{ name: {name}, signature: {{ params: [{}], ret: [{}] }}, locals: [{}], \
             start_address: {address}, end_address: {end}, file: 0 }}",
            params.join(", "),
            function.ret.join(", "),
            locals.join(", "),
        ));
        address = end;
        let declared: Vec<String> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("p{index}: {ty}"))
            .collect();
        let ret = match function.ret {
            [] => String::new(),
            [single] => format!(" -> {single}"),
            many => format!(" -> ({})", many.join(", ")),
        };
        text.push_str(&format!(
            "fn @{}({}){ret} {{\n",
            function.name,
            declared.join(", ")
        ));
        for line in function.body {
            let line = line.replace("P::", machine::P).replace("C::", machine::C);
            text.push_str(&format!("    {line}\n"));
        }
        text.push_str("}\n");
    }
    format!(
        "sst v{version}
.section(root):
.header(root):
metadata {{
    strings: [{strings}],
    functions: [{functions}],
    structs: {STRUCTS},
    types: {TYPES},
    labels: [], constants: [], main_function: 0, source_symbols: [], extra: {{}},
}}
.header(root).
.text(root):
{text}.text(root).
.section(root).
",
        version = vihaco_bytecode::VERSION,
        strings = strings.join(", "),
        functions = metadata.join(",\n        "),
    )
}

/// Parse, lower, and assemble a program through the metadata pipeline.
fn load(functions: &[Function]) -> eyre::Result<Module> {
    let source = sst(functions);
    let file = SstFile::<NoContext>::from_text(&source)?;
    let vihaco_syntax::ParsedModule { header, functions } = vihaco_syntax::ParsedModule::<
        <Machine as HasInstructionSet>::Syntax,
        Type,
        Header,
    >::parse_section(file.root())?;
    let metadata = header.0;
    let mut context = MetadataLoweringContext::new(&metadata)?;
    vihaco_syntax::ParsedModule {
        header: (),
        functions,
    }
    .lower_with_metadata(metadata, &mut context)
}

/// [`load`], with every type position parsed as a `SourceType`, so structs
/// and named types may be written by name (`struct @Pair`) in function
/// headers, `locals`, and struct fields; names resolve against the header's
/// own tables.
#[cfg(feature = "program-image-execution")]
fn load_named(functions: &[Function]) -> eyre::Result<Module> {
    use vihaco_module::module::SourceType;
    let source = sst(functions);
    let file = SstFile::<NoContext>::from_text(&source)?;
    let vihaco_syntax::ParsedModule { header, functions } = vihaco_syntax::ParsedModule::<
        <Machine as HasInstructionSet>::Syntax,
        SourceType,
        Header<SourceType>,
    >::parse_section(file.root())?;
    let metadata = header.0.resolve_types::<Type>()?;
    let mut context = MetadataLoweringContext::new(&metadata)?;
    let code = vihaco_syntax::lower_functions_with_metadata(functions, &metadata, &mut context)?;
    metadata.into_module(code)
}

/// Every diagnostic of a program, errors and warnings.
fn diagnostics(module: &Module) -> Vec<Diagnostic> {
    match verify_module(module, VerifyOptions::default()) {
        Ok(verification) => verification.warnings,
        Err(rejection) => rejection.diagnostics,
    }
}

/// Verify a program, returning its diagnostics as `(function, address, kind)`,
/// with a type mismatch's role dropped (see [`rendered`] for roles); empty
/// when it verifies without warnings.
fn verify(functions: &[Function]) -> Vec<(u32, u32, DiagnosticKind)> {
    let module = load(functions).unwrap_or_else(|error| panic!("{error:#}"));
    diagnostics(&module)
        .into_iter()
        .map(|d| {
            let kind = match d.kind {
                DiagnosticKind::TypeMismatch {
                    expected, found, ..
                } => DiagnosticKind::TypeMismatch {
                    role: None,
                    expected,
                    found,
                },
                kind => kind,
            };
            (d.function, d.address, kind)
        })
        .collect()
}

/// Verify a program, returning each diagnostic as rendered with the module's
/// names, with the build's device routes written back as `P::` and `C::`.
fn rendered(functions: &[Function]) -> Vec<String> {
    let module = load(functions).unwrap_or_else(|error| panic!("{error:#}"));
    diagnostics(&module)
        .iter()
        .map(|d| {
            d.display_with(&module)
                .to_string()
                .replace(machine::P, "P::")
                .replace(machine::C, "C::")
        })
        .collect()
}

/// A one-function program.
fn main_fn<'a>(
    params: &'a [&'a str],
    ret: &'a [&'a str],
    locals: &'a [&'a str],
    body: &'a [&'a str],
) -> Function<'a> {
    Function {
        name: "main",
        params,
        ret,
        locals,
        body,
    }
}

/// A type mismatch, from types as written (`an array` for any array).
fn mismatch(expected: &str, found: &str) -> DiagnosticKind {
    let expected = match expected {
        "an array" => Expected::AnyArray,
        ty => Type::from_text(ty).unwrap().into(),
    };
    VerifyError::type_mismatch(expected, TypeSlot::Known(Type::from_text(found).unwrap()))
        .into_kind()
}

fn underflow(needed: usize, depth: usize) -> DiagnosticKind {
    DiagnosticKind::StackUnderflow { needed, depth }
}

// ------------------------------------------------------------------ passing

#[test]
fn a_counting_loop_with_typed_locals_verifies() {
    // let mut total = 0; while n != 0 { total += n; n -= 1 } total
    let body = [
        "P::const_u64 0",   // 0
        "P::store_u64 1",   // 1
        "P::load_u64 0",    // 2: loop header
        "P::const_u64 0",   // 3
        "C::eq_u64",        // 4
        "P::cond_br 15, 6", // 5
        "P::load_u64 1",    // 6
        "P::load_u64 0",    // 7
        "C::add_u64",       // 8
        "P::store_u64 1",   // 9
        "P::load_u64 0",    // 10
        "P::const_u64 1",   // 11
        "C::sub_u64",       // 12
        "P::store_u64 0",   // 13
        "P::br 2",          // 14
        "P::load_u64 1",    // 15
        "P::ret 1",         // 16
    ];
    assert_eq!(verify(&[main_fn(&["u64"], &["u64"], &["u64"], &body)]), []);
}

#[test]
fn direct_and_indirect_calls_verify_against_signatures() {
    let main = [
        "P::const_f64 2.0",
        "P::call 1, @twice",
        "P::const_fn_ref @twice",
        "P::call_indirect (f64) -> f64",
        "P::ret 1",
    ];
    let twice = ["P::load_f64 0", "P::dup", "C::add_f64", "P::ret 1"];
    let program = [
        main_fn(&[], &["f64"], &[], &main),
        Function {
            name: "twice",
            params: &["f64"],
            ret: &["f64"],
            locals: &[],
            body: &twice,
        },
    ];
    assert_eq!(verify(&program), []);
}

#[test]
fn every_constant_pushes_its_type() {
    let body = [
        "P::const_i32 -1",
        "P::const_i64 -2",
        "P::const_u32 3",
        "P::const_u64 4",
        "P::const_f32 1.5",
        "P::const_f64 2.5",
        "P::const_bool true",
        "P::const_string \"test.sst\"",
        "P::const_fn_ref @main",
        "P::ret 9",
    ];
    let ret = [
        "i32", "i64", "u32", "u64", "f32", "f64", "bool", "str", "fn",
    ];
    assert_eq!(verify(&[main_fn(&[], &ret, &[], &body)]), []);
}

#[test]
fn bitwise_shift_logic_and_comparison_rules_follow_the_runtime() {
    let body = [
        "P::const_i64 -8",
        "P::const_u32 1",
        "C::shr_i64", // shift amounts are u32
        "P::const_i64 3",
        "C::bitand_i64",
        "P::const_i64 0",
        "C::lt_i64", // comparisons push bool
        "P::const_bool true",
        "C::xor", // logic takes bool
        "C::not",
        "P::ret 1",
    ];
    assert_eq!(verify(&[main_fn(&[], &["bool"], &[], &body)]), []);
}

#[test]
fn structs_arrays_and_untyped_locals_verify() {
    let body = [
        "P::const_i64 3",        // 0
        "P::const_f64 1.5",      // 1
        "C::struct_new @Point",  // 2
        "P::store 0",            // 3: untyped store of a struct
        "P::load 0",             // 4
        "P::const_i64 4",        // 5
        "C::set_field @Point.x", // 6
        "P::const_u64 2",        // 7
        "C::array_reserve f64",  // 8
        "P::load 0",             // 9
        "C::get_field @Point.y", // 10
        "C::array_push f64",     // 11
        "P::store 1",            // 12
        "P::load 1",             // 13
        "P::const_u64 0",        // 14
        "P::const_f64 2.5",      // 15
        "C::array_set f64",      // 16
        "P::load 1",             // 17
        "C::array_len",          // 18
        "P::const_u64 1",        // 19
        "C::eq_u64",             // 20
        "P::cond_br 22, 22",     // 21
        "P::load 0",             // 22
        "C::heap_dealloc",       // 23: a struct is a heap object
        "P::load 1",             // 24
        "P::const_u64 0",        // 25
        "C::array_get f64",      // 26
        "P::ret 1",              // 27
    ];
    let locals = ["struct 0", "array f64"];
    assert_eq!(verify(&[main_fn(&[], &["f64"], &locals, &body)]), []);
}

#[test]
fn named_types_flow_through_locals_arrays_and_wrapping() {
    let body = [
        "P::load 0",
        "C::array_new type @location, 1",
        "P::store 1",
        "P::load 1",
        "P::const_u64 0",
        "C::array_get type @location",
        "C::unwrap @location",
        "P::const_u64 7",
        "C::wrap @location",
        "C::unwrap 0",
        "C::add_u64",
        "P::ret 1",
    ];
    let program = [main_fn(&["type 0"], &["u64"], &["array type 0"], &body)];
    assert_eq!(verify(&program), []);
}

#[cfg(not(feature = "program-image-execution"))]
#[test]
fn typed_declaration_only_instructions_verify() {
    let body = [
        "C::span 0 0 0",
        "C::breakpoint",
        "C::const_string 0",
        "C::print",
        "C::halt",
    ];
    assert_eq!(verify(&[main_fn(&[], &[], &[], &body)]), []);
    let body = ["C::const_u64 1", "C::print", "C::halt"];
    assert_eq!(
        verify(&[main_fn(&[], &[], &[], &body)]),
        [(0, 1, mismatch("str", "u64"))]
    );
}

// ------------------------------------------------------------------ errors

#[test]
fn a_wrong_operand_type_is_reported() {
    let body = [
        "P::const_u64 1",
        "P::const_f64 2.0",
        "C::add_u64",
        "P::ret 1",
    ];
    assert_eq!(
        verify(&[main_fn(&[], &["u64"], &[], &body)]),
        [(0, 2, mismatch("u64", "f64"))]
    );
}

#[test]
fn conflicting_branch_types_are_reported_where_used() {
    let body = [
        "P::load_bool 0",   // 0
        "P::cond_br 2, 4",  // 1
        "P::const_u64 1",   // 2
        "P::br 5",          // 3
        "P::const_f64 1.0", // 4
        "P::const_u64 1",   // 5: join, top below is u64 | f64
        "C::add_u64",       // 6
        "P::ret 1",         // 7
    ];
    assert_eq!(
        verify(&[main_fn(&["bool"], &["u64"], &[], &body)]),
        [(
            0,
            6,
            VerifyError::type_mismatch(
                Type::U64,
                TypeSlot::Conflict {
                    first: Type::U64,
                    second: Type::F64,
                    join: Some(5)
                }
            )
            .into_kind()
        )]
    );
    // A conflict that is never used is harmless.
    let body = [
        "P::load_bool 0",
        "P::cond_br 2, 4",
        "P::const_u64 1",
        "P::br 5",
        "P::const_f64 1.0",
        "P::const_u64 1",
        "P::ret 1",
    ];
    assert_eq!(verify(&[main_fn(&["bool"], &["u64"], &[], &body)]), []);
}

#[test]
fn an_unbalanced_stack_at_a_join_is_reported() {
    let body = [
        "P::load_bool 0",  // 0
        "P::cond_br 2, 4", // 1
        "P::const_u64 1",  // 2
        "P::br 6",         // 3
        "P::const_u64 1",  // 4
        "P::const_u64 2",  // 5
        "P::ret 1",        // 6: join, depth 1 or 2
    ];
    let errors = verify(&[main_fn(&["bool"], &["u64"], &[], &body)]);
    let [
        (
            0,
            6,
            DiagnosticKind::StackDepthMismatch {
                expected, found, ..
            },
        ),
    ] = errors[..]
    else {
        panic!("{errors:?}");
    };
    assert_eq!([expected.min(found), expected.max(found)], [1, 2]);
}

#[test]
fn call_arity_is_checked_against_the_callee() {
    let helper = ["P::load_u64 0", "P::ret 1"];
    let program = |main: &'static [&'static str]| {
        verify(&[
            main_fn(&[], &["u64"], &[], main),
            Function {
                name: "helper",
                params: &["u64"],
                ret: &["u64"],
                locals: &[],
                body: &helper,
            },
        ])
    };
    assert_eq!(
        program(&[
            "P::const_u64 1",
            "P::const_u64 2",
            "P::call 2, @helper",
            "P::ret 1"
        ]),
        [(
            0,
            2,
            DiagnosticKind::CallArityMismatch {
                callee: 1,
                declared: 1,
                found: 2
            }
        )]
    );
    assert_eq!(
        program(&["P::const_f64 1.0", "P::call 1, @helper", "P::ret 1"]),
        [(0, 1, mismatch("u64", "f64"))]
    );
}

#[test]
fn return_count_and_types_are_checked_against_the_signature() {
    assert_eq!(
        verify(&[main_fn(
            &[],
            &["u64"],
            &[],
            &["P::const_f64 1.0", "P::ret 1"]
        )]),
        [(0, 1, mismatch("u64", "f64"))]
    );
    assert_eq!(
        verify(&[main_fn(
            &[],
            &["u64"],
            &[],
            &["P::const_u64 1", "P::const_u64 2", "P::ret 2"]
        )]),
        [(
            0,
            2,
            DiagnosticKind::ReturnCountMismatch {
                declared: 1,
                found: 2
            }
        )]
    );
    // Operands below the returned ones are dropped, as at runtime.
    assert_eq!(
        verify(&[main_fn(
            &[],
            &["u64"],
            &[],
            &["P::const_f64 1.0", "P::const_u64 2", "P::ret 1"]
        )]),
        []
    );
}

#[test]
fn locals_must_be_used_at_their_declared_types() {
    let errors = |locals: &'static [&'static str], body: &'static [&'static str]| {
        verify(&[main_fn(&[], &[], locals, body)])
    };
    // Typed stores and loads must name the declared type exactly.
    assert_eq!(
        errors(&["f64"], &["P::const_u64 1", "P::store_u64 0", "P::ret 0"]),
        [(0, 1, mismatch("u64", "f64"))]
    );
    assert_eq!(
        errors(&["f64"], &["P::load_i64 0", "P::ret 0"]),
        [(0, 0, mismatch("i64", "f64"))]
    );
    // A named type is not its representation, even for typed locals.
    assert_eq!(
        errors(&["type 0"], &["P::load_u64 0", "P::ret 0"]),
        [(0, 0, mismatch("u64", "type 0"))]
    );
    // An untyped store must match the declared type.
    assert_eq!(
        errors(&["struct 0"], &["P::const_u64 1", "P::store 0", "P::ret 0"]),
        [(0, 1, mismatch("struct 0", "u64"))]
    );
    // Out-of-range indices.
    assert_eq!(
        errors(&["u64"], &["P::load 5", "P::ret 0"]),
        [(
            0,
            0,
            DiagnosticKind::LocalIndexOutOfRange { index: 5, count: 1 }
        )]
    );
    assert_eq!(
        errors(&[], &["P::const_u64 1", "P::store_u64 0", "P::ret 0"]),
        [(
            0,
            1,
            DiagnosticKind::LocalIndexOutOfRange { index: 0, count: 0 }
        )]
    );
}

#[test]
fn array_elements_are_typed() {
    let body = [
        "P::const_f64 1.0",
        "C::array_new f64, 1",
        "P::const_u64 0",
        "C::array_get u64",
        "P::ret 1",
    ];
    assert_eq!(
        verify(&[main_fn(&[], &["u64"], &[], &body)]),
        [(0, 3, mismatch("array u64", "array f64"))]
    );
    let body = ["P::const_u64 1", "C::array_len", "P::ret 1"];
    assert_eq!(
        verify(&[main_fn(&[], &["u64"], &[], &body)]),
        [(0, 1, mismatch("an array", "u64"))]
    );
    // A plain `heap` reference (here an argument) is never a typed array.
    let body = [
        "P::load 0",
        "P::const_u64 0",
        "C::array_get u64",
        "P::ret 1",
    ];
    assert_eq!(
        verify(&[main_fn(&["heap"], &["u64"], &[], &body)]),
        [(0, 2, mismatch("array u64", "heap"))]
    );
}

#[test]
fn fields_are_read_from_the_named_struct_only() {
    let body = [
        "P::const_u64 1",
        "C::struct_new @Pair",
        "C::get_field @Point.x",
        "P::ret 1",
    ];
    assert_eq!(
        verify(&[main_fn(&[], &["i64"], &[], &body)]),
        [(0, 2, mismatch("struct 0", "struct 1"))]
    );
    let body = [
        "P::const_u64 1",
        "P::const_f64 1.5",
        "C::struct_new @Point",
        "P::ret 1",
    ];
    assert_eq!(
        verify(&[main_fn(&[], &["struct 0"], &[], &body)]),
        [(0, 2, mismatch("i64", "u64"))]
    );
}

#[test]
fn named_types_must_be_wrapped_and_unwrapped_explicitly() {
    let errors = |params: &'static [&'static str], body: &'static [&'static str]| {
        verify(&[main_fn(params, &[], &[], body)])
    };
    assert_eq!(
        errors(&[], &["P::const_f64 1.0", "C::wrap @location", "P::ret 0"]),
        [(0, 1, mismatch("u64", "f64"))]
    );
    // Another named type with the same representation is a different type.
    assert_eq!(
        errors(
            &[],
            &[
                "P::const_u64 1",
                "C::wrap @lane",
                "C::unwrap @location",
                "P::ret 0"
            ]
        ),
        [(0, 2, mismatch("type 0", "type 1"))]
    );
    assert_eq!(
        errors(&["type 0"], &["P::load 0", "C::add_u64", "P::ret 0"]),
        [(0, 1, underflow(2, 1))]
    );
    assert_eq!(
        errors(
            &["type 0"],
            &["P::load 0", "P::const_u64 1", "C::add_u64", "P::ret 0"]
        ),
        [(0, 2, mismatch("u64", "type 0"))]
    );
}

#[test]
fn indirect_calls_are_typed_by_their_signature_operand() {
    let target = ["P::load_u64 0", "P::ret 1"];
    let program = |main: &'static [&'static str]| {
        verify(&[
            main_fn(&[], &["u64"], &[], main),
            Function {
                name: "target",
                params: &["u64"],
                ret: &["u64"],
                locals: &[],
                body: &target,
            },
        ])
    };
    // The operand, not the referenced function, types the call: the runtime
    // checks that they agree.
    assert_eq!(
        program(&[
            "P::const_f64 1.0",
            "P::const_fn_ref @target",
            "P::call_indirect (f64) -> u64",
            "P::ret 1",
        ]),
        []
    );
    assert_eq!(
        program(&[
            "P::const_u64 1",
            "P::const_fn_ref @target",
            "P::call_indirect (f64) -> u64",
            "P::ret 1",
        ]),
        [(0, 2, mismatch("f64", "u64"))]
    );
    assert_eq!(
        program(&[
            "P::const_u64 1",
            "P::const_u64 2",
            "P::call_indirect (u64) -> u64",
            "P::ret 1",
        ]),
        [(0, 2, mismatch("fn", "u64"))]
    );
    assert_eq!(
        program(&[
            "P::const_fn_ref @target",
            "P::call_indirect (u64) -> u64",
            "P::ret 1"
        ]),
        [(0, 1, underflow(2, 1))]
    );
}

#[test]
fn indirect_call_signatures_may_name_structs_and_named_types() {
    let target = ["P::load 0", "C::unwrap @location", "P::ret 1"];
    let program = |main: &'static [&'static str]| {
        [
            main_fn(&[], &["u64"], &[], main),
            Function {
                name: "target",
                params: &["type 0"],
                ret: &["u64"],
                locals: &[],
                body: &target,
            },
        ]
    };
    let named = program(&[
        "P::const_u64 7",
        "C::wrap @location",
        "P::const_fn_ref @target",
        "P::call_indirect (type @location) -> u64",
        "P::ret 1",
    ]);
    let numeric = program(&[
        "P::const_u64 7",
        "C::wrap 0",
        "P::const_fn_ref @target",
        "P::call_indirect (type 0) -> u64",
        "P::ret 1",
    ]);
    assert_eq!(verify(&named), []);
    // Names resolve at load time to the same indices as the numeric form.
    assert_eq!(
        format!("{:?}", load(&named).unwrap().code),
        format!("{:?}", load(&numeric).unwrap().code)
    );
    let code = format!("{:?}", load(&named).unwrap().code);
    assert!(code.contains("IndirectCall((type 0) -> u64)"), "{code}");

    // A name the signature passes to the verifier is checked against the
    // tables: `lane` is a different named type, so the argument mismatches.
    let wrong = program(&[
        "P::const_u64 7",
        "C::wrap @location",
        "P::const_fn_ref @target",
        "P::call_indirect (type @lane) -> u64",
        "P::ret 1",
    ]);
    assert_eq!(verify(&wrong), [(0, 3, mismatch("type 1", "type 0"))]);

    // Structs resolve too, at any nesting depth.
    let structs = [main_fn(
        &["struct 0", "fn"],
        &["array struct 1"],
        &[],
        &[
            "P::load 0",
            "P::load 1",
            "P::call_indirect (struct @Point) -> array struct @Pair",
            "P::ret 1",
        ],
    )];
    assert_eq!(verify(&structs), []);

    // Unknown names are load-time errors.
    for (signature, message) in [
        (
            "(struct @Line) -> u64",
            "lowering @main+3 (address 3): undefined struct `@Line`",
        ),
        ("(type @zone) -> u64", "undefined named type `@zone`"),
    ] {
        let body = format!("P::call_indirect {signature}");
        let main = [
            "P::const_u64 7",
            "C::wrap @location",
            "P::const_fn_ref @target",
            body.as_str(),
            "P::ret 1",
        ];
        let error = load(&[
            main_fn(&[], &["u64"], &[], &main),
            Function {
                name: "target",
                params: &["type 0"],
                ret: &["u64"],
                locals: &[],
                body: &target,
            },
        ])
        .unwrap_err();
        assert!(format!("{error:#}").contains(message), "{error:#}");
    }

    // At runtime the resolved signature matches the target's declaration.
    #[cfg(feature = "program-image-execution")]
    {
        use vihaco_cpu::CPU;
        use vihaco_stdlib::{ExecutionState, Image, Loadable, Verified};

        let mut loaded = Loadable::new(
            Image::new(Verified::new(load(&named).unwrap()).unwrap()),
            Machine {
                execution: ExecutionState::new(),
                cpu: CPU::default(),
            },
        );
        loaded.execute(&[]).unwrap();
        assert_eq!(loaded.machine().execution.stack(), [7]);
    }
}

#[test]
fn every_error_in_a_module_is_reported_in_address_order() {
    let body = [
        "P::const_f64 1.0", // 0
        "C::add_u64",       // 1: underflow
        "P::halt",          // 2
    ];
    let other = ["P::const_u64 1", "P::ret 1"];
    let errors = verify(&[
        main_fn(&[], &[], &[], &body),
        Function {
            name: "other",
            params: &[],
            ret: &["f64"],
            locals: &[],
            body: &other,
        },
    ]);
    assert_eq!(
        errors,
        [(0, 1, underflow(2, 1)), (1, 4, mismatch("f64", "u64")),]
    );
}

// ------------------------------------------------------------------ runtime

/// A verified program can still be handed a function reference of the wrong
/// type from outside the instruction stream; `call_indirect` checks it.
#[cfg(feature = "program-image-execution")]
#[test]
fn a_host_function_reference_of_the_wrong_type_fails_at_runtime() {
    use vihaco_abi::word::encode_function_ref;
    use vihaco_cpu::CPU;
    use vihaco_stdlib::{ExecutionState, Image, Loadable, Verified};

    let main = [
        "P::const_u64 20",
        "P::load 0",
        "P::call_indirect (u64) -> u64",
        "P::ret 1",
    ];
    let inc = ["P::load_u64 0", "P::const_u64 1", "C::add_u64", "P::ret 1"];
    let negate = ["P::load_f64 0", "C::neg_f64", "P::ret 1"];
    let program = [
        main_fn(&["fn"], &["u64"], &[], &main),
        Function {
            name: "inc",
            params: &["u64"],
            ret: &["u64"],
            locals: &[],
            body: &inc,
        },
        Function {
            name: "negate",
            params: &["f64"],
            ret: &["f64"],
            locals: &[],
            body: &negate,
        },
    ];
    assert_eq!(verify(&program), []);
    let run = |function: u32| {
        let mut loaded = Loadable::new(
            Image::new(Verified::new(load(&program).unwrap()).unwrap()),
            Machine {
                execution: ExecutionState::new(),
                cpu: CPU::default(),
            },
        );
        loaded
            .execute(&[encode_function_ref(function)])
            .map(|()| loaded.machine().execution.stack().to_vec())
    };
    assert_eq!(run(1).unwrap(), [21]);
    let error = run(2).unwrap_err();
    // Located at the call, naming the callee, both types written alike.
    assert_eq!(
        error.to_string(),
        "at @main+2 (address 2) `execution::program::call_indirect (u64) -> u64`: \
         call_indirect expects a function of type (u64) -> u64, but @negate has type (f64) -> f64"
    );
}

// ------------------------------------------------------------------ messages

/// Expected text with `C::` routes, as [`rendered`] writes them in this build:
/// with `typed-declarations` alone the CPU owns both kinds of instruction,
/// so every route reads `P::`.
fn routes(text: &str) -> String {
    if machine::P == machine::C {
        text.replace("C::", "P::")
    } else {
        text.to_owned()
    }
}

/// The one diagnostic of a one-function program, rendered with names.
fn rendered_main(params: &[&str], ret: &[&str], locals: &[&str], body: &[&str]) -> String {
    let [message] = rendered(&[main_fn(params, ret, locals, body)])
        .try_into()
        .unwrap_or_else(|all: Vec<String>| panic!("{all:#?}"));
    message
}

#[test]
fn diagnostics_name_types_roles_and_locations() {
    // Point is struct 0, Pair struct 1.
    assert_eq!(
        rendered_main(
            &["struct 1"],
            &["i64"],
            &[],
            &["P::load 0", "C::get_field @Point.x", "P::ret 1"]
        ),
        routes(
            "error: operand 0 of `C::get_field`: expected struct Point, found struct Pair\n  \
         --> @main+1 (address 1): C::get_field FieldSlot { struct_id: 0, slot: 0 }"
        )
    );
    assert_eq!(
        rendered_main(
            &[],
            &[],
            &[],
            &[
                "P::const_u64 1",
                "P::const_f64 2.0",
                "C::struct_new @Point",
                "P::halt"
            ]
        ),
        routes(
            "error: field Point.x: expected i64, found u64\n  \
         --> @main+2 (address 2): C::struct_new StructLayout { struct_id: 0, field_count: 2 }"
        )
    );
    assert_eq!(
        rendered_main(&["f64"], &["u64"], &[], &["P::load_u64 0", "P::ret 1"]),
        "error: local 0 is declared f64, but `P::load_u64` accesses it as u64 (use the access \
         for f64, or the untyped one)\n  --> @main+0 (address 0): P::load_u64 0"
    );
    assert_eq!(
        rendered_main(&[], &["u64"], &[], &["P::const_f64 1.0", "P::ret 1"]),
        "error: return value 0 of @main: expected u64, found f64\n  \
         --> @main+1 (address 1): P::ret 1"
    );
    assert_eq!(
        rendered_main(&[], &[], &[], &["P::const_u64 1", "C::add_u64", "P::halt"]),
        routes(
            "error: stack underflow: `C::add_u64` needs 2 operands, but the stack has 1\n  \
         --> @main+1 (address 1): C::add_u64"
        )
    );
    let helper = ["P::load_u64 0", "P::ret 1"];
    let calls = rendered(&[
        main_fn(
            &[],
            &["u64"],
            &[],
            &["P::const_f64 1.0", "P::call 1, @helper", "P::ret 1"],
        ),
        Function {
            name: "helper",
            params: &["u64"],
            ret: &["u64"],
            locals: &[],
            body: &helper,
        },
    ]);
    assert_eq!(
        calls,
        [
            "error: argument 0 (p0) of @helper: expected u64, found f64\n  \
          --> @main+1 (address 1): P::call 1, 3"
        ]
    );
}

#[test]
fn diagnostics_at_joins_name_both_paths() {
    let unbalanced = [
        "P::load_bool 0",  // 0
        "P::cond_br 2, 4", // 1
        "P::const_u64 1",  // 2
        "P::br 6",         // 3
        "P::const_u64 1",  // 4
        "P::const_u64 2",  // 5
        "P::ret 1",        // 6: join, depth 1 or 2
    ];
    assert_eq!(
        rendered_main(&["bool"], &["u64"], &[], &unbalanced),
        "error: operand stack depth differs where paths join: 1 from @main+3 (P::br 6), 2 from \
         @main+5 (falling through)\n  --> @main+6 (address 6): P::ret 1"
    );
    let conflicting = [
        "P::load_bool 0",   // 0
        "P::cond_br 2, 4",  // 1
        "P::const_u64 1",   // 2
        "P::br 5",          // 3
        "P::const_f64 1.0", // 4
        "P::const_u64 1",   // 5: join, top below is u64 or f64
        "C::add_u64",       // 6
        "P::ret 1",         // 7
    ];
    assert_eq!(
        rendered_main(&["bool"], &["u64"], &[], &conflicting),
        routes(
            "error: operand 0 of `C::add_u64`: expected u64, found u64 or f64 (paths disagree at \
         join @main+5)\n  --> @main+6 (address 6): C::add_u64"
        )
    );
}

#[test]
fn structural_diagnostics_name_the_function_and_its_range() {
    let rendered = rendered(&[main_fn(&[], &[], &[], &["P::br 99", "P::halt"])]);
    assert_eq!(
        rendered,
        [
            "error: jump target 99 is outside @main (addresses 0..2)\n  \
             --> @main+0 (address 0): P::br 99",
            "warning: unreachable code\n  --> @main+1 (address 1): P::halt",
        ]
    );
    // The rejection lists them all, with names.
    let module = load(&[main_fn(&[], &[], &[], &["P::br 99", "P::halt"])]).unwrap();
    let rejection = verify_module(&module, VerifyOptions::default()).unwrap_err();
    let shown = rejection
        .display_with(&module)
        .to_string()
        .replace(machine::P, "P::");
    assert!(
        shown.starts_with(
            "verification failed with 1 error\nerror: jump target 99 is outside @main \
             (addresses 0..2)\n"
        ),
        "{shown}"
    );
}

/// Names are accepted in every type position: function headers, `locals`
/// and signatures in the metadata, resolved against the header's tables.
#[cfg(feature = "program-image-execution")]
#[test]
fn function_headers_and_locals_may_name_struct_types() {
    let program = [main_fn(
        &["struct @Point"],
        &["f64"],
        &["struct @Point"],
        &[
            "P::load 0",
            "P::store 1",
            "P::load 1",
            "C::get_field @Point.y",
            "P::ret 1",
        ],
    )];
    let module = load_named(&program).unwrap();
    assert_eq!(module.functions[0].signature.params[0].ty, Type::Struct(0));
    assert_eq!(
        module.functions[0].locals,
        [Type::Struct(0), Type::Struct(0)]
    );
    verify_module(&module, VerifyOptions::default()).unwrap();

    // An unknown name is reported at its metadata path.
    let typo = [main_fn(&["struct @Pointt"], &[], &[], &["P::halt"])];
    let error = load_named(&typo).unwrap_err();
    assert_eq!(
        error.to_string(),
        "metadata.functions[0].signature.params[0].ty: undefined struct `@Pointt`"
    );
}
