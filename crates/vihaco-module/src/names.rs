// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Names from a module's tables, for messages: function names and locations
//! (`@main+2`), parameter, struct, field and named-type names, and types shown
//! by name through [`TypeNames`].

use std::fmt;

use vihaco_abi::names::{SymbolNames, TypeNames};

use crate::module::{
    FunctionInfo, LabelInfo, LocalModule, NamedTypeInfo, ProgramMetadata, StructInfo,
};

/// A read-only view of a module's name-bearing tables.
///
/// Built by [`LocalModule::names`] or [`ProgramMetadata::names`]. Every lookup
/// tolerates bad indices (an unvalidated module), returning `None` rather
/// than panicking, so messages about broken modules can still be rendered.
#[derive(Debug)]
pub struct ModuleNames<'a, Ty> {
    functions: &'a [FunctionInfo<Ty>],
    structs: &'a [StructInfo<Ty>],
    types: &'a [NamedTypeInfo<Ty>],
    labels: &'a [LabelInfo],
    strings: &'a [String],
    jump_from: Option<u32>,
}

impl<Ty> Clone for ModuleNames<'_, Ty> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Ty> Copy for ModuleNames<'_, Ty> {}

impl<'a, Ty> ModuleNames<'a, Ty> {
    /// A view over tables held outside a module.
    pub fn new(
        functions: &'a [FunctionInfo<Ty>],
        structs: &'a [StructInfo<Ty>],
        types: &'a [NamedTypeInfo<Ty>],
        strings: &'a [String],
    ) -> Self {
        Self {
            functions,
            structs,
            types,
            labels: &[],
            strings,
            jump_from: None,
        }
    }

    /// Also name jump targets with `labels`.
    pub fn with_labels(mut self, labels: &'a [LabelInfo]) -> Self {
        self.labels = labels;
        self
    }

    /// Names for writing the instruction at `address`: its jump targets name
    /// only labels of the function containing it, since labels resolve per
    /// function (see [`jump_label`](Self::jump_label)). Use this to render an
    /// instruction's text so that it parses back in place.
    pub fn at(mut self, address: u32) -> Self {
        self.jump_from = Some(address);
        self
    }

    /// The label a jump target at `address` may be written as, for the
    /// instruction these names are [`at`](Self::at).
    ///
    /// That is a label at `address` when `address` lies in the function
    /// containing the instruction (or at its end), and in no other function:
    /// a label where one function ends and the next starts could belong to
    /// either, so such a target is written as its number, which always
    /// parses back. Without [`at`](Self::at), the first label at `address`.
    pub fn jump_label(&self, address: u32) -> Option<&'a str> {
        let Some(from) = self.jump_from else {
            return self.label_at(address);
        };
        let function = self.function_at(from)?;
        let contains =
            |info: &FunctionInfo<Ty>| (info.start_address..=info.end_address).contains(&address);
        let owners = self.functions.iter().filter(|info| contains(info)).count();
        if !contains(self.function(function)?) || owners != 1 {
            return None;
        }
        self.label_at(address)
    }

    /// The first function, in table order, whose first instruction is at
    /// `address`, as a direct `call` resolves it.
    pub fn function_starting_at(&self, address: u32) -> Option<u32> {
        self.functions
            .iter()
            .position(|info| info.start_address == address)
            .map(|index| index as u32)
    }

    /// The name of the first label at `address`.
    pub fn label_at(&self, address: u32) -> Option<&'a str> {
        let label = self.labels.iter().find(|label| label.address == address)?;
        self.string(label.name)
    }

    /// String-table entry `index`.
    pub fn string(&self, index: u32) -> Option<&'a str> {
        self.strings.get(index as usize).map(String::as_str)
    }

    /// Function-table entry `function`.
    pub fn function(&self, function: u32) -> Option<&'a FunctionInfo<Ty>> {
        self.functions.get(function as usize)
    }

    /// The name of function `function`, without the leading `@`.
    pub fn function_name(&self, function: u32) -> Option<&'a str> {
        self.string(self.function(function)?.name)
    }

    /// The name of parameter `index` of function `function`.
    pub fn parameter_name(&self, function: u32, index: usize) -> Option<&'a str> {
        let parameter = self.function(function)?.signature.params.get(index)?;
        self.string(parameter.name)
    }

    /// The first function, in table order, whose instructions include
    /// `address`.
    pub fn function_at(&self, address: u32) -> Option<u32> {
        self.functions
            .iter()
            .position(|info| (info.start_address..info.end_address).contains(&address))
            .map(|index| index as u32)
    }

    /// The name of struct-table entry `index`.
    pub fn struct_name(&self, index: u32) -> Option<&'a str> {
        self.string(self.structs.get(index as usize)?.name)
    }

    /// The name of field `field` of struct `struct_id`.
    pub fn field_name(&self, struct_id: u32, field: u32) -> Option<&'a str> {
        let info = self.structs.get(struct_id as usize)?;
        self.string(info.fields.get(field as usize)?.name)
    }

    /// The name of named-type-table entry `index`.
    pub fn named_type_name(&self, index: u32) -> Option<&'a str> {
        self.string(self.types.get(index as usize)?.name)
    }

    /// Where `address` is, as `@name+offset (address N)` within the function
    /// containing it (see [`function_at`](Self::function_at)), or
    /// `address N` outside every function.
    pub fn location(&self, address: u32) -> Location<'a> {
        self.location_in(self.function_at(address), address)
    }

    /// Where `address` is, relative to the start of `function` (when given
    /// and known).
    pub fn location_in(&self, function: Option<u32>, address: u32) -> Location<'a> {
        function
            .and_then(|function| {
                let info = self.function(function)?;
                let offset = address.checked_sub(info.start_address)?;
                Some(Location::in_function(
                    self.string(info.name),
                    function,
                    offset,
                    address,
                ))
            })
            .unwrap_or(Location::outside(address))
    }
}

impl<Ty> TypeNames for ModuleNames<'_, Ty> {
    fn struct_name(&self, index: u32) -> Option<&str> {
        ModuleNames::struct_name(self, index)
    }

    fn named_type_name(&self, index: u32) -> Option<&str> {
        ModuleNames::named_type_name(self, index)
    }
}

impl<Ty> SymbolNames for ModuleNames<'_, Ty> {
    fn field_name(&self, struct_id: u32, field: u32) -> Option<&str> {
        ModuleNames::field_name(self, struct_id, field)
    }

    fn function_name(&self, function: u32) -> Option<&str> {
        ModuleNames::function_name(self, function)
    }

    fn function_starting_at(&self, address: u32) -> Option<u32> {
        ModuleNames::function_starting_at(self, address)
    }

    fn label_at(&self, address: u32) -> Option<&str> {
        self.jump_label(address)
    }

    fn string(&self, index: u32) -> Option<&str> {
        ModuleNames::string(self, index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionName<'a> {
    Named(&'a str),
    Index(u32),
}

/// An instruction address, shown relative to its function:
/// `@main+2 (address 2)`. Built by [`ModuleNames::location`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    function: Option<(FunctionName<'a>, u32)>,
    address: u32,
}

impl<'a> Location<'a> {
    /// `address`, at `offset` within function `function` (shown by `name`
    /// when given): `@main+2 (address 2)` or `function 0+2 (address 2)`.
    pub fn in_function(name: Option<&'a str>, function: u32, offset: u32, address: u32) -> Self {
        let name = match name {
            Some(name) => FunctionName::Named(name),
            None => FunctionName::Index(function),
        };
        Location {
            function: Some((name, offset)),
            address,
        }
    }

    /// `address`, outside every known function: `address 7`.
    pub fn outside(address: u32) -> Self {
        Location {
            function: None,
            address,
        }
    }

    /// The section-local instruction address.
    pub fn address(&self) -> u32 {
        self.address
    }

    /// The location without the address: `@main+2`, `function 0+2`, or
    /// `address 7` outside every function.
    pub fn short(&self) -> impl fmt::Display + '_ {
        struct Short<'l, 'a>(&'l Location<'a>);
        impl fmt::Display for Short<'_, '_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0.function {
                    Some((FunctionName::Named(name), offset)) => write!(f, "@{name}+{offset}"),
                    Some((FunctionName::Index(index), offset)) => {
                        write!(f, "function {index}+{offset}")
                    }
                    None => write!(f, "address {}", self.0.address),
                }
            }
        }
        Short(self)
    }
}

impl fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.function {
            Some((FunctionName::Named(name), offset)) => {
                write!(f, "@{name}+{offset} (address {})", self.address)
            }
            Some((FunctionName::Index(index), offset)) => {
                write!(f, "function {index}+{offset} (address {})", self.address)
            }
            None => write!(f, "address {}", self.address),
        }
    }
}

impl<I, V, Ty, Info> LocalModule<I, V, Ty, Info> {
    /// Names from this module's tables, for messages.
    pub fn names(&self) -> ModuleNames<'_, Ty> {
        ModuleNames::new(&self.functions, &self.structs, &self.types, &self.strings)
            .with_labels(&self.labels)
    }
}

impl<V, Ty, Info> ProgramMetadata<V, Ty, Info> {
    /// Names from these tables, for messages.
    pub fn names(&self) -> ModuleNames<'_, Ty> {
        ModuleNames::new(&self.functions, &self.structs, &self.types, &self.strings)
            .with_labels(&self.labels)
    }
}

impl<I, V, Ty, Info> TypeNames for LocalModule<I, V, Ty, Info> {
    fn struct_name(&self, index: u32) -> Option<&str> {
        self.names().struct_name(index)
    }

    fn named_type_name(&self, index: u32) -> Option<&str> {
        self.names().named_type_name(index)
    }
}

impl<I, V, Ty, Info> SymbolNames for LocalModule<I, V, Ty, Info> {
    fn field_name(&self, struct_id: u32, field: u32) -> Option<&str> {
        self.names().field_name(struct_id, field)
    }

    fn function_name(&self, function: u32) -> Option<&str> {
        self.names().function_name(function)
    }

    fn function_starting_at(&self, address: u32) -> Option<u32> {
        self.names().function_starting_at(address)
    }

    fn label_at(&self, address: u32) -> Option<&str> {
        self.names().label_at(address)
    }

    fn string(&self, index: u32) -> Option<&str> {
        self.names().string(index)
    }
}

impl<V, Ty, Info> SymbolNames for ProgramMetadata<V, Ty, Info> {
    fn field_name(&self, struct_id: u32, field: u32) -> Option<&str> {
        self.names().field_name(struct_id, field)
    }

    fn function_name(&self, function: u32) -> Option<&str> {
        self.names().function_name(function)
    }

    fn function_starting_at(&self, address: u32) -> Option<u32> {
        self.names().function_starting_at(address)
    }

    fn label_at(&self, address: u32) -> Option<&str> {
        self.names().label_at(address)
    }

    fn string(&self, index: u32) -> Option<&str> {
        self.names().string(index)
    }
}

impl<V, Ty, Info> TypeNames for ProgramMetadata<V, Ty, Info> {
    fn struct_name(&self, index: u32) -> Option<&str> {
        self.names().struct_name(index)
    }

    fn named_type_name(&self, index: u32) -> Option<&str> {
        self.names().named_type_name(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{FieldInfo, Parameter, Signature};
    use vihaco_abi::{Type, Value};

    fn module() -> LocalModule<(), Value, Type> {
        let function = |name, start, end| FunctionInfo {
            name,
            signature: Signature {
                params: vec![Parameter {
                    name: 2,
                    ty: Type::U64,
                }],
                ret: vec![],
            },
            locals: vec![Type::U64],
            start_address: start,
            end_address: end,
            file: 0,
        };
        LocalModule {
            code: vec![(); 5],
            functions: vec![function(0, 0, 3), function(1, 3, 5), function(9, 5, 5)],
            structs: vec![StructInfo {
                name: 3,
                fields: vec![FieldInfo {
                    name: 4,
                    ty: Type::I64,
                }],
            }],
            strings: ["main", "helper", "p0", "Point", "x"]
                .map(String::from)
                .to_vec(),
            ..LocalModule::default()
        }
    }

    #[test]
    fn locations_are_relative_to_the_containing_function() {
        let module = module();
        let names = module.names();
        assert_eq!(names.location(2).to_string(), "@main+2 (address 2)");
        assert_eq!(names.location(4).to_string(), "@helper+1 (address 4)");
        assert_eq!(names.location(7).to_string(), "address 7");
        assert_eq!(
            names.location_in(Some(2), 5).to_string(),
            "function 2+0 (address 5)"
        );
    }

    #[test]
    fn names_resolve_through_the_string_table() {
        let module = module();
        let names = module.names();
        assert_eq!(names.function_name(1), Some("helper"));
        assert_eq!(names.parameter_name(0, 0), Some("p0"));
        assert_eq!(names.parameter_name(0, 1), None);
        assert_eq!(names.struct_name(0), Some("Point"));
        assert_eq!(names.field_name(0, 0), Some("x"));
        assert_eq!(names.field_name(0, 1), None);
        assert_eq!(
            Type::array(Type::Struct(0))
                .display_with(&module)
                .to_string(),
            "array struct Point"
        );
    }
}
