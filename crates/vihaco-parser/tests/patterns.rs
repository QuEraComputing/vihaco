// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use chumsky::{IterParser, Parser, primitive::just};
use vihaco_parser::Parse;
use vihaco_parser_core::Parse as ParseTrait;

fn parse<'src, T>(source: &'src str) -> Result<T, Vec<chumsky::error::Simple<'src, char>>>
where
    T: ParseTrait<'src>,
{
    T::parser().parse(source).into_result()
}

#[derive(Clone, Debug, PartialEq)]
struct Operand(String);

impl<'src> ParseTrait<'src> for Operand {
    fn parser()
    -> impl Parser<'src, &'src str, Self, chumsky::extra::Err<chumsky::error::Simple<'src, char>>>
    {
        vihaco_parser_core::ident().map(Operand)
    }
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum PermutedTuple {
    #[pattern = "'p012 $0 $1 $2"]
    P012(i64, bool, String),
    #[pattern = "'p021 $0 $2 $1"]
    P021(i64, bool, String),
    #[pattern = "'p102 $1 $0 $2"]
    P102(i64, bool, String),
    #[pattern = "'p120 $1 $2 $0"]
    P120(i64, bool, String),
    #[pattern = "'p201 $2 $0 $1"]
    P201(i64, bool, String),
    #[pattern = "'p210 $2 $1 $0"]
    P210(i64, bool, String),
}

#[test]
fn tuple_bindings_are_assigned_by_index_not_capture_order() {
    assert_eq!(
        parse("test::p012 7 true word"),
        Ok(PermutedTuple::P012(7, true, "word".into()))
    );
    assert_eq!(
        parse("test::p021 7 word true"),
        Ok(PermutedTuple::P021(7, true, "word".into()))
    );
    assert_eq!(
        parse("test::p102 true 7 word"),
        Ok(PermutedTuple::P102(7, true, "word".into()))
    );
    assert_eq!(
        parse("test::p120 true word 7"),
        Ok(PermutedTuple::P120(7, true, "word".into()))
    );
    assert_eq!(
        parse("test::p201 word 7 true"),
        Ok(PermutedTuple::P201(7, true, "word".into()))
    );
    assert_eq!(
        parse("test::p210 word true 7"),
        Ok(PermutedTuple::P210(7, true, "word".into()))
    );
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$right `,` $left"]
struct PermutedNamed {
    left: i64,
    right: bool,
}

#[test]
fn named_bindings_are_assigned_by_name_not_capture_order() {
    assert_eq!(
        parse("true, 42"),
        Ok(PermutedNamed {
            left: 42,
            right: true,
        })
    );
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum Punctuation {
    #[pattern = "'comma $0 `,` $1"]
    Comma(i64, bool),
    #[pattern = "'at $0 `@` $1"]
    At(i64, String),
    #[pattern = "'wrapped `before` $0 `after`"]
    Wrapped(i64),
}

#[test]
fn comma_suppresses_only_leading_whitespace() {
    assert_eq!(
        parse("test::comma 1, true"),
        Ok(Punctuation::Comma(1, true))
    );
    assert_eq!(
        parse("test::comma   1,    false"),
        Ok(Punctuation::Comma(1, false))
    );

    assert!(parse::<Punctuation>("test::comma 1 , true").is_err());
    assert!(parse::<Punctuation>("test::comma 1,true").is_err());
}

#[test]
fn at_suppresses_only_trailing_whitespace() {
    assert_eq!(
        parse("test::at 1 @target"),
        Ok(Punctuation::At(1, "target".into()))
    );
    assert_eq!(
        parse("test::at 1    @target"),
        Ok(Punctuation::At(1, "target".into()))
    );

    assert!(parse::<Punctuation>("test::at 1@target").is_err());
    assert!(parse::<Punctuation>("test::at 1 @ target").is_err());
}

#[test]
fn ordinary_atoms_require_ascii_spaces_and_exact_literals() {
    assert_eq!(
        parse("test::wrapped before 9 after"),
        Ok(Punctuation::Wrapped(9))
    );
    assert_eq!(
        parse("test::wrapped   before    9  after"),
        Ok(Punctuation::Wrapped(9))
    );

    for invalid in [
        "test::wrapped before9 after",
        "test::wrapped before 9after",
        "test::wrapped\tbefore 9 after",
        "test::wrapped before\n9 after",
        "test::wrapped wrong 9 after",
        "test::wrapped before 9 wrong",
        "prefix test::wrapped before 9 after",
        "test::wrapped before 9 after suffix",
        "test::wrapped before nope after",
    ] {
        assert!(
            parse::<Punctuation>(invalid).is_err(),
            "accepted {invalid:?}"
        );
    }
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum GeneratedInstruction {
    Halt,
    Move(i64, bool),
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum ExplicitInstruction {
    #[pattern = "'halt"]
    Halt,
    #[pattern = "'move $0 `,` $1"]
    Move(i64, bool),
}

#[test]
fn generated_instruction_patterns_match_equivalent_explicit_patterns() {
    assert_eq!(parse("test::halt"), Ok(GeneratedInstruction::Halt));
    assert_eq!(parse("test::halt"), Ok(ExplicitInstruction::Halt));
    assert_eq!(
        parse("test::move 12, true"),
        Ok(GeneratedInstruction::Move(12, true))
    );
    assert_eq!(
        parse("test::move 12, true"),
        Ok(ExplicitInstruction::Move(12, true))
    );
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "analog")]
enum AnalogDialect {
    #[pattern = "'set $0"]
    Set(i64),
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "digital")]
enum DigitalDialect {
    #[pattern = "'set $0"]
    Set(i64),
}

#[test]
fn instruction_heads_select_the_dialect() {
    assert_eq!(parse("analog::set 3"), Ok(AnalogDialect::Set(3)));
    assert_eq!(parse("digital::set 5"), Ok(DigitalDialect::Set(5)));

    assert!(parse::<AnalogDialect>("digital::set 3").is_err());
    assert!(parse::<DigitalDialect>("analog::set 5").is_err());
    assert!(parse::<AnalogDialect>("set 3").is_err());
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
enum GeneratedValue {
    Nothing,
    Number(i64),
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
enum ExplicitValue {
    #[pattern = "`nothing`"]
    Nothing,
    #[pattern = "$0"]
    Number(i64),
}

#[test]
fn generated_value_patterns_match_equivalent_explicit_patterns() {
    assert_eq!(parse("nothing"), Ok(GeneratedValue::Nothing));
    assert_eq!(parse("nothing"), Ok(ExplicitValue::Nothing));
    assert_eq!(parse("17"), Ok(GeneratedValue::Number(17)));
    assert_eq!(parse("17"), Ok(ExplicitValue::Number(17)));
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
struct GeneratedNamedValue {
    value: i64,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$value"]
struct ExplicitNamedValue {
    value: i64,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
struct RawIdentifierField {
    r#type: i64,
}

#[test]
fn generated_named_field_pattern_matches_an_explicit_pattern() {
    assert_eq!(parse("23"), Ok(GeneratedNamedValue { value: 23 }));
    assert_eq!(parse("23"), Ok(ExplicitNamedValue { value: 23 }));
    assert_eq!(parse("29"), Ok(RawIdentifierField { r#type: 29 }));
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    #[pattern = "'load $0"]
    Load(i64),
    #[pattern = "'store $0 `,` $1"]
    Store(Operand, i64),
    #[pattern = "'jump $0 `@` $1"]
    Jump(i64, String),
    Halt,
}

fn instruction_list<'src>() -> impl Parser<
    'src,
    &'src str,
    Vec<Instruction>,
    chumsky::extra::Err<chumsky::error::Simple<'src, char>>,
> {
    Instruction::parser()
        .separated_by(just('\n'))
        .allow_trailing()
        .collect()
}

#[test]
fn parses_a_newline_separated_instruction_source() {
    let source = "test::load 4\ntest::store destination, 8\ntest::jump 2 @loop\ntest::halt\n";

    assert_eq!(
        instruction_list().parse(source).into_result(),
        Ok(vec![
            Instruction::Load(4),
            Instruction::Store(Operand("destination".into()), 8),
            Instruction::Jump(2, "loop".into()),
            Instruction::Halt,
        ])
    );
}

#[test]
fn instruction_list_rejects_a_bad_instruction_without_losing_neighbors() {
    let source = "test::load 4\ntest::store destination 8\ntest::halt";
    assert!(instruction_list().parse(source).has_errors());
}

#[derive(Clone, Debug, PartialEq)]
struct Marker<'a>(PhantomData<&'a ()>);

impl<'src, 'a> ParseTrait<'src> for Marker<'a> {
    fn parser()
    -> impl Parser<'src, &'src str, Self, chumsky::extra::Err<chumsky::error::Simple<'src, char>>>
    {
        just("marker").to(Marker(PhantomData))
    }
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$0 $1"]
struct LifetimeCollision<'__vihaco_src>(i64, Marker<'__vihaco_src>);

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$0"]
struct Generic<T>(T)
where
    T: for<'a> ParseTrait<'a>;

#[test]
fn generics_and_generated_lifetime_name_collisions_compile_and_parse() {
    assert_eq!(
        parse("31 marker"),
        Ok(LifetimeCollision(31, Marker(PhantomData)))
    );
    assert_eq!(parse("37"), Ok(Generic(37_i64)));
}

macro_rules! define_instruction_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Parse, Debug, PartialEq)]
        #[syntax_class(instruction, head = "test")]
        enum $name {
            $($variant),+
        }
    };
}

define_instruction_enum!(OneVariant { V0 });
define_instruction_enum!(TwoVariants { V0, V1 });
define_instruction_enum!(TwentySixVariants {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
    V9,
    V10,
    V11,
    V12,
    V13,
    V14,
    V15,
    V16,
    V17,
    V18,
    V19,
    V20,
    V21,
    V22,
    V23,
    V24,
    V25,
});
define_instruction_enum!(TwentySevenVariants {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
    V9,
    V10,
    V11,
    V12,
    V13,
    V14,
    V15,
    V16,
    V17,
    V18,
    V19,
    V20,
    V21,
    V22,
    V23,
    V24,
    V25,
    V26,
});
define_instruction_enum!(FiftyThreeVariants {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
    V9,
    V10,
    V11,
    V12,
    V13,
    V14,
    V15,
    V16,
    V17,
    V18,
    V19,
    V20,
    V21,
    V22,
    V23,
    V24,
    V25,
    V26,
    V27,
    V28,
    V29,
    V30,
    V31,
    V32,
    V33,
    V34,
    V35,
    V36,
    V37,
    V38,
    V39,
    V40,
    V41,
    V42,
    V43,
    V44,
    V45,
    V46,
    V47,
    V48,
    V49,
    V50,
    V51,
    V52,
});

#[test]
fn enum_choice_boundaries_compile_and_select_the_right_variant() {
    assert_eq!(parse("test::v0"), Ok(OneVariant::V0));
    assert_eq!(parse("test::v1"), Ok(TwoVariants::V1));
    assert_eq!(parse("test::v25"), Ok(TwentySixVariants::V25));
    assert_eq!(parse("test::v26"), Ok(TwentySevenVariants::V26));
    assert_eq!(parse("test::v52"), Ok(FiftyThreeVariants::V52));
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum AcronymInstruction {
    HttpServer,
}

#[test]
fn generated_names_are_lowercase() {
    assert_eq!(
        parse("test::httpserver"),
        Ok(AcronymInstruction::HttpServer)
    );
    assert!(parse::<AcronymInstruction>("test::HttpServer").is_err());
}
