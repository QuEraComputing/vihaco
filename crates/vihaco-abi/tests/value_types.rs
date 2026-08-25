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
