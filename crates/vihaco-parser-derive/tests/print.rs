// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! `#[derive(Parse)]` also derives `Print`; these tests pin `parse(print(x)) == x`
//! for every pattern feature, plus the exact canonical spelling.

use std::fmt::Debug;

use chumsky::Parser;
use vihaco_parser::{BareToken, Ident, Parse as ParseTrait, Print, QuotedString};
use vihaco_parser_derive::Parse;

/// Print `value`, reparse the text, and return it after checking equality.
fn round_trip<T>(value: &T) -> String
where
    T: Print + Debug + PartialEq + for<'src> ParseTrait<'src>,
{
    let source = value.to_source().unwrap();
    let parsed = T::parser().parse(source.as_str()).into_result();
    assert_eq!(parsed.as_ref(), Ok(value), "reparsing {source:?}");
    let layout = outside_strings(&source);
    assert_eq!(layout.trim(), layout, "no surrounding whitespace");
    assert!(!layout.contains("  "), "single spaces: {source:?}");
    assert!(!layout.contains('\n'), "single line: {source:?}");
    source
}

/// The printed text with string literal contents removed (escapes included).
fn outside_strings(source: &str) -> String {
    let mut layout = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in source.chars() {
        if quoted {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
        } else {
            quoted = c == '"';
            layout.push(c);
        }
    }
    layout
}

fn ident(value: &str) -> Ident {
    Ident(value.to_owned())
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
enum Instruction {
    Halt,
    #[pattern = "'load $0"]
    Load(i64),
    Move(i64, bool),
    #[pattern = "'swap $1 $0"]
    Swap(i64, Ident),
    #[pattern = "'store $0 `,` $1"]
    Store(Ident, f64),
    #[pattern = "'jump $0 `@` $1"]
    Jump(u32, Ident),
    #[pattern = "'br `@` $0 `,` `@` $1"]
    Branch(Ident, Ident),
    #[pattern = "'wrapped `before` $0 `after`"]
    Wrapped(i64),
    #[pattern = "'named $right:u64 `,` $left"]
    Named {
        left: u32,
        right: u64,
    },
    #[pattern = "'nothing"]
    EmptyTuple(),
    #[pattern = "'list $0:Vec<(u32, f64)>"]
    List(Vec<(u32, f64)>),
    #[pattern = "'text $0 $1"]
    Text(QuotedString, String),
    #[pattern = "'maybe $0"]
    Maybe(Option<u32>),
    // Prefix of `halt`; longer mnemonics are tried first when parsing.
    #[pattern = "'hal"]
    Hal,
}

#[test]
fn instruction_patterns_round_trip_with_canonical_spacing() {
    let cases = [
        (Instruction::Halt, "test.halt"),
        (Instruction::Hal, "test.hal"),
        (Instruction::Load(-42), "test.load -42"),
        (Instruction::Move(3, true), "test.move 3, true"),
        (Instruction::Swap(7, ident("x")), "test.swap x 7"),
        (
            Instruction::Store(ident("dst"), -0.5),
            "test.store dst, -0.5",
        ),
        (Instruction::Jump(1, ident("loop")), "test.jump 1 @loop"),
        (
            Instruction::Branch(ident("then"), ident("else")),
            "test.br @then, @else",
        ),
        (Instruction::Wrapped(9), "test.wrapped before 9 after"),
        (
            Instruction::Named {
                left: 1,
                right: u64::MAX,
            },
            "test.named 18446744073709551615, 1",
        ),
        (Instruction::EmptyTuple(), "test.nothing"),
        (
            Instruction::List(vec![(1, 2.0), (3, 1e-7)]),
            "test.list [(1, 2.0), (3, 1e-7)]",
        ),
        (Instruction::List(vec![]), "test.list []"),
        (
            Instruction::Text(QuotedString("a \"b\"\n".into()), "\\".into()),
            r#"test.text "a \"b\"\n" "\\""#,
        ),
        (Instruction::Maybe(None), "test.maybe null"),
        (Instruction::Maybe(Some(4)), "test.maybe 4"),
    ];
    for (value, expected) in cases {
        assert_eq!(round_trip(&value), expected);
    }
}

#[test]
fn unprintable_operands_fail_instead_of_writing_bad_source() {
    assert!(Instruction::Load(0).to_source().is_ok());
    assert!(Instruction::Store(ident("two words"), 1.0)
        .to_source()
        .is_err());
    assert!(Instruction::Store(ident("x"), f64::NAN)
        .to_source()
        .is_err());
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
enum Operand {
    #[pattern = "$0"]
    Number(u32),
    #[pattern = "`@` $0"]
    Symbol(Ident),
    #[pattern = "$0"]
    Text(QuotedString),
    Nothing,
    #[pattern = "`pair` `(` $0 `,` $1 `)`"]
    Pair(i64, i64),
    #[pattern = "`list` `[` $items `]`"]
    Items {
        items: Vec<u32>,
    },
    #[pattern = "`block` `{` $0 `,` $1 `}`"]
    Block(u32, u32),
    #[pattern = "`key` $0 `:` $1"]
    Keyed(Ident, BareToken),
    #[pattern = "`label` `:` $0"]
    Label(u32),
}

#[test]
fn value_patterns_cover_every_punctuation_literal() {
    let cases = [
        (Operand::Number(5), "5"),
        (Operand::Symbol(ident("main")), "@main"),
        (Operand::Text(QuotedString("hi".into())), "\"hi\""),
        (Operand::Nothing, "nothing"),
        (Operand::Pair(-1, 2), "pair(-1, 2)"),
        (
            Operand::Items {
                items: vec![1, 2, 3],
            },
            "list[[1, 2, 3]]",
        ),
        (Operand::Block(1, 2), "block{1, 2}"),
        // A capture before `:` gets a space so a token cannot absorb the colon.
        (
            Operand::Keyed(ident("a:b"), BareToken("@c".into())),
            "key a:b : @c",
        ),
        (Operand::Label(3), "label: 3"),
    ];
    for (value, expected) in cases {
        assert_eq!(round_trip(&value), expected);
    }
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(type)]
enum Prim {
    #[pattern = "`int`"]
    Int,
    #[pattern = "`bool`"]
    Bool,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(type)]
enum Ty {
    #[pattern = "$0"]
    Prim(Prim),
    #[pattern = "`array` `[` $0 `,` $1 `]`"]
    Array(Prim, u32),
    #[pattern = "`fn` `(` $0 `)`"]
    Function(Vec<Prim>),
}

#[test]
fn type_patterns_round_trip() {
    let cases = [
        (Ty::Prim(Prim::Int), "int"),
        (Ty::Array(Prim::Bool, 4), "array[bool, 4]"),
        (Ty::Function(vec![Prim::Int, Prim::Bool]), "fn([int, bool])"),
        (Ty::Function(vec![]), "fn([])"),
    ];
    for (value, expected) in cases {
        assert_eq!(round_trip(&value), expected);
    }
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata, head = "device")]
enum DeviceHeader {
    #[pattern = "module.setting $0 `,` $1"]
    ModuleSetting(i64, i64),
    Generic,
    #[pattern = "channel $name `:` $value"]
    Channel {
        name: Ident,
        value: f64,
    },
}

#[test]
fn headed_metadata_patterns_round_trip() {
    assert_eq!(
        round_trip(&DeviceHeader::ModuleSetting(3, -4)),
        "device module.setting 3, -4"
    );
    assert_eq!(round_trip(&DeviceHeader::Generic), "device generic");
    assert_eq!(
        round_trip(&DeviceHeader::Channel {
            name: ident("ch0"),
            value: 2.5,
        }),
        "device channel ch0 : 2.5"
    );
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata, open = "{", close = "}")]
struct Signature {
    params: Vec<u32>,
    ret: Vec<u32>,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata, open = "{", close = "}")]
struct Function {
    name: u32,
    signature: Signature,
    r#type: Option<u32>,
    größe: QuotedString,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata, head = "metadata", open = "<", close = ">")]
struct Header {
    strings: Vec<String>,
    functions: Vec<Function>,
    main: Option<u32>,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata, open = "{", close = "}")]
struct Empty {}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(metadata)]
#[pattern = "`{` `}`"]
struct NoInfo;

#[test]
fn delimited_records_print_keys_in_declaration_order() {
    let function = Function {
        name: 1,
        signature: Signature {
            params: vec![],
            ret: vec![7],
        },
        r#type: None,
        größe: QuotedString("é".into()),
    };
    assert_eq!(
        round_trip(&function),
        r#"{ name: 1, signature: { params: [], ret: [7] }, type: null, "größe": "é" }"#
    );
    let header = Header {
        strings: vec!["main".into(), "a \"quoted\" name".into()],
        functions: vec![function],
        main: Some(0),
    };
    assert_eq!(
        round_trip(&header),
        concat!(
            r#"metadata < strings: ["main", "a \"quoted\" name"], "#,
            r#"functions: [{ name: 1, signature: { params: [], ret: [7] }, "#,
            r#"type: null, "größe": "é" }], main: 0 >"#
        )
    );
    assert_eq!(round_trip(&Empty {}), "{}");
    assert_eq!(round_trip(&NoInfo), "{}");
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "$right `,` $left"]
struct PermutedNamed {
    left: i64,
    right: bool,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "test")]
#[pattern = "'pair $1 $0"]
struct PermutedTuple(i64, bool);

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
struct Wrapper {
    value: i64,
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
struct Newtype(u64);

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
struct Marker;

#[test]
fn structs_print_bindings_by_field_not_capture_order() {
    assert_eq!(
        round_trip(&PermutedNamed {
            left: 4,
            right: false
        }),
        "false, 4"
    );
    assert_eq!(round_trip(&PermutedTuple(-3, true)), "test.pair true -3");
    assert_eq!(round_trip(&Wrapper { value: -1 }), "-1");
    assert_eq!(round_trip(&Newtype(0)), "0");
    assert_eq!(round_trip(&Marker), "marker");
}

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(value)]
#[pattern = "`wrap` $0"]
struct Generic<T>(T)
where
    T: for<'a> ParseTrait<'a>;

#[derive(Parse, Debug, PartialEq)]
#[syntax_class(instruction, head = "generic")]
enum GenericInstruction<T, U>
where
    T: for<'a> ParseTrait<'a>,
    U: for<'a> ParseTrait<'a> + 'static,
{
    #[pattern = "'value $0 `,` $1"]
    Value(T, Vec<U>),
}

#[test]
fn generic_types_print_when_their_operands_do() {
    assert_eq!(round_trip(&Generic(-5_i64)), "wrap -5");
    assert_eq!(round_trip(&Generic(ident("x"))), "wrap x");
    assert_eq!(
        round_trip(&GenericInstruction::Value(1_u32, vec![true])),
        "generic.value 1, [true]"
    );
}

/// A small deterministic generator (no extra dependencies) for broad coverage.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn float(&mut self) -> f64 {
        loop {
            let value = f64::from_bits(self.next() << 11 | self.next() & 0x7ff);
            if value.is_finite() {
                return value;
            }
        }
    }

    fn word(&mut self) -> String {
        const ALPHABET: &[u8] = b"abcXYZ019_.:-+@/";
        let length = 1 + self.below(6) as usize;
        let text: String = (0..length)
            .map(|_| ALPHABET[self.below(ALPHABET.len() as u64) as usize] as char)
            .collect();
        // Identifiers reject `@` and a leading `//` comment marker.
        let text = text.replace('@', "a");
        if text.starts_with("//") {
            format!("x{text}")
        } else {
            text
        }
    }

    fn text(&mut self) -> String {
        const CHARS: &[char] = &['a', ' ', '"', '\\', '\n', '\t', '\r', '\0', 'é', '\'', '}'];
        let length = self.below(8) as usize;
        (0..length)
            .map(|_| CHARS[self.below(CHARS.len() as u64) as usize])
            .collect()
    }

    fn instruction(&mut self) -> Instruction {
        match self.below(14) {
            0 => Instruction::Halt,
            1 => Instruction::Load(self.next() as i64),
            2 => Instruction::Move(self.next() as i64 - (1 << 52), self.below(2) == 0),
            3 => Instruction::Swap(-(self.next() as i64), ident(&self.word())),
            4 => Instruction::Store(ident(&self.word()), self.float()),
            5 => Instruction::Jump(self.next() as u32, ident(&self.word())),
            6 => Instruction::Branch(ident(&self.word()), ident(&self.word())),
            7 => Instruction::Wrapped(self.next() as i64),
            8 => Instruction::Named {
                left: self.next() as u32,
                right: self.next(),
            },
            9 => Instruction::EmptyTuple(),
            10 => Instruction::List(
                (0..self.below(4))
                    .map(|_| (self.next() as u32, self.float()))
                    .collect(),
            ),
            11 => Instruction::Text(QuotedString(self.text()), self.text()),
            12 => Instruction::Maybe((self.below(2) == 0).then(|| self.next() as u32)),
            _ => Instruction::Hal,
        }
    }
}

#[test]
fn generated_instructions_round_trip() {
    let mut rng = Lcg(0x5eed);
    for _ in 0..2000 {
        round_trip(&rng.instruction());
    }
}

#[test]
fn generated_records_round_trip() {
    let mut rng = Lcg(0xfeed);
    for _ in 0..500 {
        let function = Function {
            name: rng.next() as u32,
            signature: Signature {
                params: (0..rng.below(3)).map(|_| rng.next() as u32).collect(),
                ret: (0..rng.below(3)).map(|_| rng.next() as u32).collect(),
            },
            r#type: (rng.below(2) == 0).then(|| rng.next() as u32),
            größe: QuotedString(rng.text()),
        };
        let header = Header {
            strings: (0..rng.below(3)).map(|_| rng.text()).collect(),
            functions: vec![function],
            main: (rng.below(2) == 0).then(|| rng.next() as u32),
        };
        round_trip(&header);
    }
}
