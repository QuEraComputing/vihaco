// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Struct-operand checks for the legacy enum build.
//!
//! The typed dialects resolve every struct operand against the struct table
//! while lowering, so `struct_new` always allocates the table's field count.
//! The legacy enum has no lowering step: `struct_new $0, $1` carries a field
//! count written by the author, and the CPU never sees the struct table. An
//! embedder that assembles a legacy module with a struct table should call
//! [`check_struct_operands`] once, before running it; code that skips the
//! check is unchecked, and the CPU then trusts the written counts.

use crate::RuntimeInstruction;
use vihaco_abi::Type;
use vihaco_module::module::StructInfo;

/// Check every struct operand in `code` against the module's struct table.
///
/// - `struct_new $0, $1`: struct `$0` exists and `$1` is its field count.
/// - `get_field` / `set_field $0, $1`: struct `$0` exists and has slot `$1`.
/// - array element types (`array_new struct 3, …`): every struct they name
///   exists, including structs nested in array element types.
///
/// # Errors
/// The first mismatch, located by instruction address.
pub fn check_struct_operands<Ty>(
    code: &[RuntimeInstruction],
    structs: &[StructInfo<Ty>],
) -> eyre::Result<()> {
    let field_count = |struct_id: u32| {
        structs
            .get(struct_id as usize)
            .map(|info| info.fields.len())
            .ok_or_else(|| {
                eyre::eyre!(
                    "struct {struct_id} is outside the struct table ({} structs)",
                    structs.len()
                )
            })
    };
    let element = |elem: &Type| {
        let mut result = Ok(());
        elem.visit_struct_refs(&mut |struct_id| {
            if result.is_ok() {
                result = field_count(struct_id).map(drop);
            }
        });
        result
    };
    for (address, instruction) in code.iter().enumerate() {
        use RuntimeInstruction::*;
        let result = match instruction {
            StructNew(struct_id, count) => field_count(*struct_id).and_then(|fields| {
                eyre::ensure!(
                    *count as usize == fields,
                    "struct {struct_id} has {fields} fields, but the instruction allocates {count}"
                );
                Ok(())
            }),
            GetField(struct_id, slot) | SetField(struct_id, slot) => field_count(*struct_id)
                .and_then(|fields| {
                    eyre::ensure!(
                        (*slot as usize) < fields,
                        "struct {struct_id} has no field slot {slot} ({fields} fields)"
                    );
                    Ok(())
                }),
            ArrayNew(elem, _)
            | ArrayGet(elem)
            | ArraySet(elem)
            | ArrayReserve(elem)
            | ArrayPush(elem) => element(elem),
            _ => Ok(()),
        };
        result
            .map_err(|error| error.wrap_err(format!("instruction {address} ({instruction:?})")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use RuntimeInstruction::*;
    use vihaco_module::module::FieldInfo;

    fn point() -> Vec<StructInfo<Type>> {
        let field = |name| FieldInfo {
            name,
            ty: Type::U64,
        };
        vec![StructInfo {
            name: 0,
            fields: vec![field(1), field(2)],
        }]
    }

    #[test]
    fn struct_new_must_allocate_the_table_field_count() {
        let structs = point();
        check_struct_operands(&[StructNew(0, 2), GetField(0, 1), SetField(0, 0)], &structs)
            .unwrap();
        for (code, message) in [
            (
                StructNew(0, 5),
                "struct 0 has 2 fields, but the instruction allocates 5",
            ),
            (StructNew(0, 1), "allocates 1"),
            (
                StructNew(99, 0),
                "struct 99 is outside the struct table (1 structs)",
            ),
            (GetField(0, 2), "struct 0 has no field slot 2"),
            (SetField(3, 0), "struct 3 is outside"),
            (ArrayNew(Type::Struct(99), 1), "struct 99 is outside"),
            (
                ArrayGet(Type::array(Type::Struct(1))),
                "struct 1 is outside",
            ),
        ] {
            let err = check_struct_operands(&[Halt, code], &structs).unwrap_err();
            let rendered = format!("{err:#}");
            assert!(rendered.contains(message), "{rendered}");
            assert!(rendered.starts_with("instruction 1 ("), "{rendered}");
        }
        // With an empty struct table, no struct operand is valid.
        assert!(check_struct_operands::<Type>(&[StructNew(0, 0)], &[]).is_err());
    }
}
