// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use vihaco_abi::instruction::{FromBytes, OpCode, WriteBytes};
use vihaco_abi::traits::FromText;
use vihaco_abi::{Type, Value};

fn encoded<T>(value: &T) -> Vec<u8>
where
    T: WriteBytes,
{
    let mut bytes = Vec::new();
    value.write_bytes(&mut bytes).unwrap();
    bytes
}

#[test]
fn new_values_have_types_and_rust_conversions() {
    let values = [Value::from(-7_i32), Value::from(1.5_f32)];
    assert_eq!(values[0].type_of(), Type::I32);
    assert_eq!(values[1].type_of(), Type::F32);
    assert_eq!(i32::try_from(values[0]).unwrap(), -7);
    assert_eq!(f32::try_from(values[1]).unwrap(), 1.5);
    assert_eq!(Value::I32(-7).cast(Type::I64).unwrap(), Value::I64(-7));
    assert_eq!(Value::F32(1.5).cast(Type::F64).unwrap(), Value::F64(1.5));
}

#[test]
fn new_values_round_trip_through_binary_encoding() {
    let value = Value::I32(i32::MIN);
    let bytes = encoded(&value);
    let decoded = Value::from_bytes(&mut std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(decoded, value);

    let value = Value::F32(f32::from_bits(0x7fc0_1234));
    let bytes = encoded(&value);
    let decoded = Value::from_bytes(&mut std::io::Cursor::new(bytes)).unwrap();
    assert!(matches!(decoded, Value::F32(value) if value.to_bits() == 0x7fc0_1234));

    assert_eq!(encoded(&Type::I32), vec![0x09]);
    assert_eq!(encoded(&Type::F32), vec![0x0A]);
}

#[test]
fn new_values_parse_and_display() {
    assert_eq!(Type::from_text("i32").unwrap(), Type::I32);
    assert_eq!(Type::from_text("f32").unwrap(), Type::F32);
    assert_eq!(Value::from_text("i32 -7").unwrap(), Value::I32(-7));
    assert_eq!(Value::from_text("f32 1.5").unwrap(), Value::F32(1.5));
    assert_eq!(Type::I32.to_string(), "i32");
    assert_eq!(Type::F32.to_string(), "f32");
}

#[test]
fn scalar_instruction_traits_use_narrow_widths() {
    assert_eq!(i32::width(), 4);
    assert_eq!(f32::width(), 4);
    assert_eq!(encoded(&1.0_f32), 1.0_f32.to_le_bytes());
    assert_eq!(encoded(&-7_i32), (-7_i32).to_le_bytes());
}

/// Bitwise equality, so NaN payloads and signed zeros are compared exactly.
fn same_value(left: Value, right: Value) -> bool {
    match (left, right) {
        (Value::F64(l), Value::F64(r)) => l.to_bits() == r.to_bits(),
        (Value::F32(l), Value::F32(r)) => l.to_bits() == r.to_bits(),
        (l, r) => l == r,
    }
}

fn reparse_value(value: Value) -> (String, Value) {
    use chumsky::Parser as _;
    use vihaco_parser::{Parse, Print};
    let source = value.to_source().unwrap();
    let parsed = Value::parser()
        .parse(source.as_str())
        .into_result()
        .unwrap();
    (source, parsed)
}

#[test]
fn values_print_as_source_that_reparses_bitwise() {
    let values = [
        Value::Undefined,
        Value::String(3),
        Value::Bool(true),
        Value::Bool(false),
        Value::I64(i64::MIN),
        Value::I64(-1),
        Value::I32(i32::MIN),
        Value::U32(u32::MAX),
        Value::U64(u64::MAX),
        Value::F64(-0.0),
        Value::F64(1.5),
        Value::F64(-1.9999999999998004e-6),
        Value::F64(1e300),
        Value::F64(f64::INFINITY),
        Value::F64(f64::NEG_INFINITY),
        Value::F64(f64::from_bits(0x7ff8_0000_dead_beef)),
        Value::F32(-0.0),
        Value::F32(3.25e-12),
        Value::F32(f32::INFINITY),
        Value::F32(f32::from_bits(0x7fc0_1234)),
        Value::FunctionRef(7),
        Value::HeapRef(0),
    ];
    for value in values {
        let (source, parsed) = reparse_value(value);
        assert!(same_value(value, parsed), "{source}: {parsed:?}");
        assert!(!source.contains("  ") && source.trim() == source);
    }
    assert_eq!(reparse_value(Value::U64(3)).0, "u64 3");
    assert_eq!(reparse_value(Value::F64(-0.5)).0, "f64 -0.5");
    assert_eq!(
        reparse_value(Value::F32(f32::from_bits(0x7fc0_1234))).0,
        format!("f32_bits {}", 0x7fc0_1234_u32)
    );
}

#[test]
fn types_print_their_canonical_spelling() {
    use chumsky::Parser as _;
    use vihaco_parser::{Parse, Print};
    let types = [
        (Type::Undefined, "undefined"),
        (Type::String, "str"),
        (Type::Bool, "bool"),
        (Type::I64, "i64"),
        (Type::I32, "i32"),
        (Type::U32, "u32"),
        (Type::U64, "u64"),
        (Type::F64, "f64"),
        (Type::F32, "f32"),
        (Type::FunctionRef, "fn"),
        (Type::HeapRef, "heap"),
    ];
    for (ty, spelling) in types {
        assert_eq!(ty.to_source().unwrap(), spelling);
        assert_eq!(Type::parser().parse(spelling).into_result(), Ok(ty));
    }
    let list = vec![Type::U64, Type::FunctionRef];
    assert_eq!(list.to_source().unwrap(), "[u64, fn]");
    assert_eq!(
        <Vec<Type>>::parser().parse("[u64, fn]").into_result(),
        Ok(list)
    );
}
