// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Parsed-syntax data shapes. See module docs in [`super`].

use vihaco_module::module::{ResolveType, TypeLookup};
use vihaco_parser::Ident;

/// Parsed SST module containing consumer instruction values.
///
/// `I` is the consumer's instruction syntax type, `Ty` is its source type
/// syntax, and `H` is its section-header type.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModule<I, Ty, H> {
    pub header: H,
    pub functions: Vec<ParsedFunction<I, Ty>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFunction<I, Ty> {
    /// Function name with the leading `@` stripped (`@main` → `"main"`).
    pub name: Ident,
    /// Function parameters parsed from `name: type` declarations.
    pub params: Vec<Param<Ty>>,
    /// Return types parsed with the consumer-provided `Ty` syntax.
    pub return_types: Vec<Ty>,
    pub body: Vec<I>,
    /// Labels and their positions in the executable body, for loader preparation.
    #[cfg(feature = "typed-declarations")]
    pub labels: Vec<ParsedLabel>,
}

/// A function-local label that occupies no executable instruction slot.
#[cfg(feature = "typed-declarations")]
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLabel {
    pub name: Ident,
    /// Index of the next instruction in this function, possibly `body.len()`.
    pub instruction_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param<Ty> {
    pub name: Ident,
    pub ty: Ty,
}

impl<I, S, H> ParsedModule<I, S, H> {
    /// Check `fn` declarations against supplied metadata without changing either.
    /// Table bounds and runtime constant references are checked during assembly.
    ///
    /// Declared types are resolved against the metadata's tables first (see
    /// [`ResolveType`]), so a header written with
    /// [`SourceType`](vihaco_module::module::SourceType), such as
    /// `fn @f(p: struct @Point)`, matches metadata declaring `struct 0`.
    pub fn validate_metadata<V, Ty, Info>(
        &self,
        metadata: &vihaco_module::module::ProgramMetadata<V, Ty, Info>,
    ) -> eyre::Result<()>
    where
        S: ResolveType<Ty>,
        Ty: PartialEq + std::fmt::Display,
    {
        validate_functions(&self.functions, metadata)
    }
}

/// `(a, b)`, for messages about a list of types.
fn type_list<Ty: std::fmt::Display>(types: &[Ty]) -> String {
    let types: Vec<String> = types.iter().map(ToString::to_string).collect();
    format!("({})", types.join(", "))
}

/// Check function declarations independently of the consumer's header shape.
pub(crate) fn validate_functions<I, S, Ty, V, Info>(
    functions: &[ParsedFunction<I, S>],
    metadata: &vihaco_module::module::ProgramMetadata<V, Ty, Info>,
) -> eyre::Result<()>
where
    S: ResolveType<Ty>,
    Ty: PartialEq + std::fmt::Display,
{
    eyre::ensure!(
        functions.len() == metadata.functions.len(),
        "the section declares {} functions, but metadata.functions has {} entries",
        functions.len(),
        metadata.functions.len()
    );
    let lookup = TypeLookup::new(&metadata.structs, &metadata.types, &metadata.strings);
    let mut seen = std::collections::BTreeSet::new();
    let mut address = 0usize;
    for function in functions {
        let name = function.name.as_str();
        eyre::ensure!(
            seen.insert(name),
            "duplicate function declaration `@{name}`"
        );
        let mut matches = metadata.functions.iter().filter(|info| {
            metadata
                .strings
                .get(info.name as usize)
                .is_some_and(|value| value == name)
        });
        let info = matches
            .next()
            .ok_or_else(|| eyre::eyre!("function `@{name}` has no metadata"))?;
        eyre::ensure!(
            matches.next().is_none(),
            "function `@{name}` has duplicate metadata"
        );
        let end = address
            .checked_add(function.body.len())
            .ok_or_else(|| eyre::eyre!("instruction count overflow"))?;
        eyre::ensure!(
            info.start_address as usize == address && info.end_address as usize == end,
            "function `@{name}`: supplied range {}..{} disagrees with body range {address}..{end}",
            info.start_address,
            info.end_address
        );
        let resolve = |ty: &S, what: &dyn Fn() -> String| {
            ty.resolve_type(&lookup)
                .map_err(|error| eyre::eyre!("function `@{name}`: {}: {error}", what()))
        };
        eyre::ensure!(
            function.params.len() == info.signature.params.len(),
            "function `@{name}` declares {} parameters, but its metadata declares {}",
            function.params.len(),
            info.signature.params.len()
        );
        for (index, (parameter, declared)) in function
            .params
            .iter()
            .zip(&info.signature.params)
            .enumerate()
        {
            let ty = resolve(&parameter.ty, &|| format!("parameter {index}"))?;
            let declared_name = metadata
                .strings
                .get(declared.name as usize)
                .map_or("<invalid name>", String::as_str);
            eyre::ensure!(
                declared_name == parameter.name.as_str() && ty == declared.ty,
                "function `@{name}`: parameter {index} is `{}: {ty}`, but its metadata declares \
                 `{declared_name}: {}`",
                parameter.name,
                declared.ty
            );
        }
        let returns = function
            .return_types
            .iter()
            .enumerate()
            .map(|(index, ty)| resolve(ty, &|| format!("result {index}")))
            .collect::<eyre::Result<Vec<Ty>>>()?;
        eyre::ensure!(
            returns == info.signature.ret,
            "function `@{name}` returns {}, but its metadata declares {}",
            type_list(&returns),
            type_list(&info.signature.ret)
        );
        #[cfg(feature = "typed-declarations")]
        {
            let mut labels = std::collections::BTreeSet::new();
            for label in &function.labels {
                eyre::ensure!(
                    labels.insert(label.name.as_str()),
                    "function `@{name}`: duplicate label `@{}`",
                    label.name
                );
                eyre::ensure!(
                    label.instruction_index <= function.body.len(),
                    "function `@{name}`: label `@{}` is outside its body",
                    label.name
                );
                let label_address = address + label.instruction_index;
                eyre::ensure!(
                    metadata.labels.iter().any(|info| {
                        metadata
                            .strings
                            .get(info.name as usize)
                            .is_some_and(|value| value == label.name.as_str())
                            && info.address as usize == label_address
                    }),
                    "function `@{name}`: label `@{}` at {label_address} requires matching metadata",
                    label.name
                );
            }
        }
        address = end;
    }
    Ok(())
}
