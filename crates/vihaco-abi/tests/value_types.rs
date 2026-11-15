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
fn struct_types_round_trip_through_text_and_bytes() {
    let ty = Type::Struct(258);
    assert_eq!(encoded(&ty), vec![0x0B, 0x02, 0x01, 0x00, 0x00]);
    let decoded = Type::from_bytes(&mut std::io::Cursor::new(encoded(&ty))).unwrap();
    assert_eq!(decoded, ty);

    assert_eq!(ty.to_string(), "struct 258");
    assert_eq!(Type::from_text("struct 258").unwrap(), ty);
    // `struct` shares a prefix with `str`; both must still parse.
    assert_eq!(Type::from_text("str").unwrap(), Type::String);
    assert!(Type::from_text("struct").is_err());
    assert!(Type::from_text("struct x").is_err());
}

#[test]
fn array_types_nest_and_round_trip_through_text_and_bytes() {
    let cases = [
        (Type::array(Type::F64), "array f64", vec![0x0C, 0x06]),
        (
            Type::array(Type::array(Type::U64)),
            "array array u64",
            vec![0x0C, 0x0C, 0x05],
        ),
        (
            Type::array(Type::Struct(2)),
            "array struct 2",
            vec![0x0C, 0x0B, 0x02, 0x00, 0x00, 0x00],
        ),
    ];
    for (ty, text, bytes) in cases {
        assert_eq!(ty.to_string(), text);
        assert_eq!(Type::from_text(text).unwrap(), ty);
        assert_eq!(encoded(&ty), bytes);
        let decoded = Type::from_bytes(&mut std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(decoded, ty);
    }
    assert!(Type::from_text("array").is_err());
    assert!(Type::from_text("array nope").is_err());
    // A truncated element type is an error, not a panic.
    assert!(Type::from_bytes(&mut std::io::Cursor::new(vec![0x0C])).is_err());
}

#[test]
fn array_nesting_is_limited_in_bytes_and_text() {
    let max = Type::MAX_ARRAY_DEPTH;
    let deepest = Type::nested_array(max, Type::I64).unwrap();
    assert_eq!(deepest.array_depth(), max);

    // At the limit: decodes and parses.
    let mut bytes = vec![0x0C; max];
    bytes.push(0x03);
    assert_eq!(
        Type::from_bytes(&mut std::io::Cursor::new(bytes)).unwrap(),
        deepest
    );
    let text = format!("{}i64", "array ".repeat(max));
    assert_eq!(Type::from_text(&text).unwrap(), deepest);

    // One past the limit: an error, in both forms.
    let mut bytes = vec![0x0C; max + 1];
    bytes.push(0x03);
    let err = Type::from_bytes(&mut std::io::Cursor::new(bytes)).unwrap_err();
    assert!(err.to_string().contains("nested more than 32"), "{err}");
    assert!(Type::from_text(&format!("{}i64", "array ".repeat(max + 1))).is_err());
    assert!(Type::nested_array(1, deepest).is_err());

    // A hostile run of array bytes is rejected without overflowing the stack.
    let mut bytes = vec![0x0C; 2_000_000];
    bytes.push(0x03);
    assert!(Type::from_bytes(&mut std::io::Cursor::new(bytes)).is_err());
    let text = format!("{}i64", "array ".repeat(200_000));
    assert!(Type::from_text(&text).is_err());
}

#[test]
fn struct_refs_are_found_inside_array_element_types() {
    let mut found = Vec::new();
    Type::array(Type::array(Type::Struct(3))).visit_struct_refs(&mut |index| found.push(index));
    Type::F64.visit_struct_refs(&mut |index| found.push(index));
    assert_eq!(found, [3]);
}

#[test]
fn scalar_instruction_traits_use_narrow_widths() {
    assert_eq!(i32::width(), 4);
    assert_eq!(f32::width(), 4);
    assert_eq!(encoded(&1.0_f32), 1.0_f32.to_le_bytes());
    assert_eq!(encoded(&-7_i32), (-7_i32).to_le_bytes());
}
