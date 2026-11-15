// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Raising a loaded module back to source: the name lookups instructions
//! use to [`Raise`](vihaco_abi::Raise) themselves, and the standard header
//! rebuilt from the module's tables.
//!
//! Raising is the inverse of lowering. Where lowering resolved `@name`
//! operands to addresses and indices and interned string literals, raising
//! looks the names and the text up again. The capability traits mirror the
//! lowering ones ([`LabelNames`] for `LabelLookup`, [`FunctionNames`] for
//! `FunctionLookup`, [`StringTable`] for `StringIntern`), so dialect `Raise`
//! implementations stay generic over the context. [`RaiseContext`]
//! implements all three for a loaded [`LocalModule`].

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use crate::module::{
    ConstantDirective, ConstantValue, FunctionDirective, LocalModule, NoInfo, ProgramDirective,
    Symbol,
};

/// Name the symbol at an instruction address, for branch targets.
pub trait LabelNames {
    /// A symbol defined at `address`.
    ///
    /// # Errors
    /// Fails if no symbol is defined there.
    fn label_name(&self, address: u32) -> eyre::Result<&str>;
}

/// Name functions, for calls and function references.
///
/// Calls hold an executable address; reference constants hold a
/// function-table index. These are separate lookups, as in lowering.
pub trait FunctionNames {
    /// The name of function-table entry `index`.
    fn function_name(&self, index: u32) -> eyre::Result<&str>;
    /// The name of a function that starts at `address`.
    fn function_at(&self, address: u32) -> eyre::Result<&str>;
}

/// Read the section's string table, for string operands.
pub trait StringTable {
    /// The text of `strings[index]`.
    fn string(&self, index: u32) -> eyre::Result<&str>;
}

/// Name lookups backed by a loaded module: its symbol table, function
/// table, and string table.
///
/// Every lookup only reads the tables; nothing is invented. A label lookup
/// for an address without a symbol fails and is remembered, so a
/// disassembler can raise once, read [`unlabeled`](Self::unlabeled), add
/// symbols for those addresses, and raise again with
/// [`with_symbols`](Self::with_symbols).
///
/// When several symbols share an address, the first one in table order
/// names it; likewise for functions that start at the same address.
#[derive(Debug, Clone)]
pub struct RaiseContext {
    strings: Vec<String>,
    functions: Vec<String>,
    function_starts: HashMap<u32, usize>,
    labels: HashMap<u32, String>,
    unlabeled: RefCell<BTreeSet<u32>>,
}

impl RaiseContext {
    /// Lookups for `module`, naming addresses with its own symbol table.
    ///
    /// # Errors
    /// Fails if a function's name is not in the string table.
    pub fn new<I, V, Ty, Info>(module: &LocalModule<I, V, Ty, Info>) -> eyre::Result<Self> {
        Self::with_symbols(module, &module.symbols)
    }

    /// Lookups for `module`, naming addresses with `symbols` instead of the
    /// module's symbol table, for example the module's symbols plus
    /// synthesized labels.
    ///
    /// # Errors
    /// Fails if a function's name is not in the string table.
    pub fn with_symbols<I, V, Ty, Info>(
        module: &LocalModule<I, V, Ty, Info>,
        symbols: &[Symbol],
    ) -> eyre::Result<Self> {
        let mut functions = Vec::with_capacity(module.functions.len());
        let mut function_starts = HashMap::new();
        for (index, function) in module.functions.iter().enumerate() {
            let name = module.strings.get(function.name as usize).ok_or_else(|| {
                eyre::eyre!(
                    "functions[{index}].name: index {} is outside the string table of length {}",
                    function.name,
                    module.strings.len()
                )
            })?;
            functions.push(name.clone());
            function_starts
                .entry(function.start_address)
                .or_insert(index);
        }
        let mut labels = HashMap::with_capacity(symbols.len());
        for symbol in symbols {
            labels
                .entry(symbol.address)
                .or_insert_with(|| symbol.name.clone());
        }
        Ok(Self {
            strings: module.strings.clone(),
            functions,
            function_starts,
            labels,
            unlabeled: RefCell::new(BTreeSet::new()),
        })
    }

    /// Every address a [`label_name`](LabelNames::label_name) lookup asked
    /// for that has no symbol, in ascending order.
    pub fn unlabeled(&self) -> Vec<u32> {
        self.unlabeled.borrow().iter().copied().collect()
    }
}

impl LabelNames for RaiseContext {
    fn label_name(&self, address: u32) -> eyre::Result<&str> {
        match self.labels.get(&address) {
            Some(name) => Ok(name),
            None => {
                self.unlabeled.borrow_mut().insert(address);
                Err(eyre::eyre!(
                    "no symbol is defined at address {address:#06x}"
                ))
            }
        }
    }
}

impl FunctionNames for RaiseContext {
    fn function_name(&self, index: u32) -> eyre::Result<&str> {
        self.functions
            .get(index as usize)
            .map(String::as_str)
            .ok_or_else(|| {
                eyre::eyre!(
                    "function index {index} is outside the function table of length {}",
                    self.functions.len()
                )
            })
    }

    fn function_at(&self, address: u32) -> eyre::Result<&str> {
        self.function_starts
            .get(&address)
            .map(|&index| self.functions[index].as_str())
            .ok_or_else(|| eyre::eyre!("no function starts at address {address:#06x}"))
    }
}

impl StringTable for RaiseContext {
    fn string(&self, index: u32) -> eyre::Result<&str> {
        self.strings
            .get(index as usize)
            .map(String::as_str)
            .ok_or_else(|| {
                eyre::eyre!(
                    "string index {index} is outside the string table of length {}",
                    self.strings.len()
                )
            })
    }
}

/// Decode a literal word that lowering encoded, requiring that encoding it
/// again gives back the same word.
///
/// Raising must be faithful: a word such as a `bool` other than `0` or `1`
/// has no literal that lowers back to it, so `what` names the literal kind in
/// the error.
pub fn decode_canonical<T: Copy>(
    word: u64,
    decode: impl FnOnce(u64) -> T,
    encode: impl FnOnce(T) -> u64,
    what: &str,
) -> eyre::Result<T> {
    let value = decode(word);
    eyre::ensure!(
        encode(value) == word,
        "word {word:#x} is not the encoding of any {what} literal"
    );
    Ok(value)
}

/// Application header directives that describe a module's `extra`
/// metadata: the inverse of the application's header collection.
///
/// A disassembler writes these before the standard directives. `NoInfo`
/// has none.
pub trait ExtraDirectives<D> {
    /// The directives, in header order.
    fn extra_directives(&self) -> eyre::Result<Vec<D>>;
}

impl<D> ExtraDirectives<D> for NoInfo {
    fn extra_directives(&self) -> eyre::Result<Vec<D>> {
        Ok(Vec::new())
    }
}

/// The standard header directives that describe `module`'s tables.
///
/// In order: one `.file` before each run of consecutive `.func`s that share
/// a file (none for a leading run in the empty file `""`), one `.func` per
/// function in table order, the `.entry`, one `.const` per constant (string
/// and function constants by their text and name), and the
/// `.source_symbol`s. Assembling these directives yields the module's
/// functions, constants, entry, and source symbols again, and a string
/// table that starts with the same strings when the module was loaded from
/// SST.
///
/// # Errors
/// Fails if a table refers to a string or function that does not exist.
pub fn header_directives<I, V, Ty, Info>(
    module: &LocalModule<I, V, Ty, Info>,
) -> eyre::Result<Vec<ProgramDirective<V, Ty>>>
where
    V: ConstantValue + Clone,
    Ty: Clone,
{
    let string = |index: u32, what: &dyn Fn() -> String| {
        module.strings.get(index as usize).cloned().ok_or_else(|| {
            eyre::eyre!(
                "{}: string index {index} is outside the string table of length {}",
                what(),
                module.strings.len()
            )
        })
    };
    let function_name = |index: u32, what: &dyn Fn() -> String| {
        let function = module.functions.get(index as usize).ok_or_else(|| {
            eyre::eyre!(
                "{}: function index {index} is outside the function table of length {}",
                what(),
                module.functions.len()
            )
        })?;
        string(function.name, &|| format!("functions[{index}].name"))
    };

    let mut directives = Vec::new();
    let mut file = String::new();
    for (index, function) in module.functions.iter().enumerate() {
        let path = |field: &str| format!("functions[{index}].{field}");
        let function_file = string(function.file, &|| path("file"))?;
        if function_file != file {
            directives.push(ProgramDirective::File(function_file.clone()));
            file = function_file;
        }
        let params = function
            .signature
            .params
            .iter()
            .map(|param| Ok((string(param.name, &|| path("params"))?, param.ty.clone())))
            .collect::<eyre::Result<_>>()?;
        directives.push(ProgramDirective::Function(FunctionDirective {
            name: string(function.name, &|| path("name"))?,
            params,
            ret: function.signature.ret.clone(),
            locals: function.local_count,
        }));
    }
    if let Some(main) = module.main_function {
        directives.push(ProgramDirective::Entry(function_name(main, &|| {
            "main_function".to_owned()
        })?));
    }
    for (index, constant) in module.constants.iter().enumerate() {
        let what = || format!("constants[{index}]");
        let directive = if let Some(id) = constant.string_index() {
            ConstantDirective::Str(string(id, &what)?)
        } else if let Some(id) = constant.function_index() {
            ConstantDirective::Function(function_name(id, &what)?)
        } else {
            ConstantDirective::Value(constant.clone())
        };
        directives.push(ProgramDirective::Const(directive));
    }
    directives.extend(
        module
            .source_symbols
            .iter()
            .cloned()
            .map(ProgramDirective::SourceSymbol),
    );
    Ok(directives)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{FunctionInfo, Parameter, ProgramHeader, Signature, SourceSymbolInfo};
    use vihaco_abi::{Type, Value};

    fn function(name: u32, file: u32, start: u32, end: u32) -> FunctionInfo<Type> {
        FunctionInfo {
            name,
            signature: Signature {
                params: vec![Parameter {
                    name: 2,
                    ty: Type::U64,
                }],
                ret: vec![Type::U64],
            },
            local_count: 1,
            start_address: start,
            end_address: end,
            file,
        }
    }

    fn module() -> LocalModule<(), Value, Type> {
        LocalModule {
            code: vec![(); 4],
            functions: vec![function(0, 3, 2, 4), function(1, 3, 0, 2)],
            symbols: vec![
                Symbol {
                    name: "main".into(),
                    address: 0,
                },
                Symbol {
                    name: "loop".into(),
                    address: 1,
                },
                Symbol {
                    name: "helper".into(),
                    address: 2,
                },
            ],
            constants: vec![Value::U64(7), Value::String(4), Value::FunctionRef(0)],
            strings: ["helper", "main", "x", "a.sst", "hi"]
                .map(String::from)
                .to_vec(),
            main_function: Some(1),
            source_symbols: vec![SourceSymbolInfo {
                index: 9,
                name: "user".into(),
            }],
            extra: NoInfo,
        }
    }

    #[test]
    fn lookups_name_addresses_functions_and_strings() -> eyre::Result<()> {
        let module = module();
        let context = RaiseContext::new(&module)?;
        assert_eq!(context.label_name(1)?, "loop");
        assert_eq!(context.label_name(0)?, "main");
        assert_eq!(context.function_name(0)?, "helper");
        assert_eq!(context.function_at(0)?, "main");
        assert_eq!(context.string(4)?, "hi");
        assert!(context.function_at(1).is_err());
        assert!(context.string(5).is_err());
        assert!(context.function_name(2).is_err());
        assert!(context.label_name(3).is_err());
        assert!(context.label_name(9).is_err());
        assert_eq!(context.unlabeled(), [3, 9]);

        let extra = [Symbol {
            name: "L0003".into(),
            address: 3,
        }];
        let mut symbols = module.symbols.clone();
        symbols.extend(extra);
        let context = RaiseContext::with_symbols(&module, &symbols)?;
        assert_eq!(context.label_name(3)?, "L0003");
        assert!(context.unlabeled().is_empty());
        Ok(())
    }

    #[test]
    fn the_rebuilt_header_assembles_to_the_same_tables() -> eyre::Result<()> {
        let module = module();
        let directives = header_directives(&module)?;
        assert!(matches!(&directives[0], ProgramDirective::File(file) if file == "a.sst"));
        let metadata = directives
            .into_iter()
            .collect::<ProgramHeader>()
            .finish(NoInfo)?;
        assert_eq!(metadata.strings, module.strings);
        assert_eq!(metadata.constants, module.constants);
        assert_eq!(metadata.main_function, module.main_function);
        assert_eq!(metadata.source_symbols, module.source_symbols);
        let functions = metadata.resolve_functions(&module.symbols, module.code.len())?;
        assert_eq!(functions, module.functions);
        Ok(())
    }

    #[test]
    fn files_are_written_once_per_run_and_reset_to_the_empty_file() -> eyre::Result<()> {
        let mut module = module();
        module.strings.push(String::new());
        module.functions[0].file = 5;
        module.functions.push(function(1, 3, 0, 2));
        let files: Vec<String> = header_directives(&module)?
            .into_iter()
            .filter_map(|directive| match directive {
                ProgramDirective::File(file) => Some(file),
                _ => None,
            })
            .collect();
        assert_eq!(files, ["a.sst"]);
        module.functions.swap(0, 1);
        let files: Vec<String> = header_directives(&module)?
            .into_iter()
            .filter_map(|directive| match directive {
                ProgramDirective::File(file) => Some(file),
                _ => None,
            })
            .collect();
        assert_eq!(files, ["a.sst", "", "a.sst"]);
        Ok(())
    }

    #[test]
    fn literal_words_must_be_canonical() {
        let bool_word = |word| {
            decode_canonical(
                word,
                vihaco_abi::word::decode_bool,
                vihaco_abi::word::encode_bool,
                "bool",
            )
        };
        assert!(bool_word(1).unwrap());
        assert!(!bool_word(0).unwrap());
        let error = bool_word(5).unwrap_err();
        assert_eq!(
            error.to_string(),
            "word 0x5 is not the encoding of any bool literal"
        );
    }
}
