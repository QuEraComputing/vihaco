// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{
    attr::{EnumAttrs, PatternInfo, StructAttrs, SyntaxClassAttr, VariantAttrs},
    codegen::{
        BindingRef::{Field, Index},
        PatternAtom::{Binding, Literal, Token},
        PatternLiteral::{Keyword, Symbol},
    },
};
use chumsky::{
    error::Rich,
    extra,
    primitive::{any, choice, just},
    recursive::recursive,
    text, IterParser, Parser,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt::{self, Display},
    vec,
};
use syn::{
    ext::IdentExt, spanned::Spanned, Attribute, Data, DeriveInput, Error, Fields, FieldsNamed,
    FieldsUnnamed, Generics, Ident, Lifetime, Result, Type,
};

/// Paths used by emitted code in the consuming crate. Standalone derives use
/// direct dependencies; dialect parsers use the framework's reexports.
struct ParserPaths {
    parser_root: syn::Path,
    chumsky: syn::Path,
}

impl ParserPaths {
    fn parser_for(&self, ty: &Type) -> TokenStream2 {
        let parser_root = &self.parser_root;
        quote!(<#ty as #parser_root::Parse>::parser())
    }
}

#[derive(PartialEq)]
enum BindingRef<'p> {
    Index(u32, Option<&'p str>),
    Field(&'p str, Option<&'p str>),
}

#[derive(PartialEq)]
enum PatternLiteral<'p> {
    Keyword(&'p str),
    Symbol(char),
}

impl PatternLiteral<'_> {
    fn suppresses_leading_whitespace(&self) -> bool {
        matches!(
            self,
            Symbol(',' | ':' | ';' | '{' | '}' | '[' | ']' | '(' | ')')
        )
    }

    fn suppresses_trailing_whitespace(&self) -> bool {
        matches!(
            self,
            Symbol('@' | ':' | ';' | '{' | '}' | '[' | ']' | '(' | ')')
        )
    }
}

impl Display for PatternLiteral<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Keyword(s) => write!(f, "{s}"),
            Symbol(c) => write!(f, "{c}"),
        }
    }
}

trait BindingState {}

impl BindingState for BindingRef<'_> {}
impl BindingState for ResolvedBinding {}

#[derive(PartialEq)]
enum PatternAtom<'p, Binding: BindingState> {
    Token(&'p str),
    Binding(Binding),
    Literal(PatternLiteral<'p>),
}

impl<'p, B> PatternAtom<'p, B>
where
    B: BindingState,
{
    fn token(&self) -> Option<&'p str> {
        match self {
            Token(s) => Some(s),
            _ => None,
        }
    }
}

impl<'p> PatternAtom<'p, BindingRef<'p>> {
    fn resolve<F>(self, f: F) -> eyre::Result<PatternAtom<'p, ResolvedBinding>>
    where
        F: Fn(BindingRef<'p>) -> eyre::Result<ResolvedBinding>,
    {
        match self {
            Binding(b) => Ok(Binding(f(b)?)),
            Token(t) => Ok(Token(t)),
            Literal(l) => Ok(Literal(l)),
        }
    }
}

fn pattern_syntax_parser<'p>(
    metadata: bool,
) -> impl Parser<'p, &'p str, Vec<PatternAtom<'p, BindingRef<'p>>>, extra::Err<Rich<'p, char>>> {
    let ident = text::ascii::ident();
    let digits = text::int(10);

    let token = just('\'').ignore_then(ident).map(Token).boxed();

    let binding_index = digits
        .to_slice()
        .try_map(|s: &str, span| {
            s.parse::<u32>()
                .map_err(|_| Rich::custom(span, "binding index must be a valid number"))
        })
        .map(|index| Index(index, None))
        .boxed();

    let binding_field = ident.map(|name| Field(name, None)).boxed();

    // Delimited Rust types may contain spaces, commas, and nested type syntax.
    // Parenthesize types needing whitespace outside delimiters, e.g. (dyn Trait).
    let nested_type = recursive(|inner| {
        choice((
            just("->").ignored(),
            inner.clone().delimited_by(just('<'), just('>')),
            inner.clone().delimited_by(just('('), just(')')),
            inner.clone().delimited_by(just('['), just(']')),
            inner.delimited_by(just('{'), just('}')),
            any()
                .filter(|ch: &char| !"$`<>()[]{}".contains(*ch))
                .ignored(),
        ))
        .repeated()
        .collect::<()>()
    });
    let parser_type = choice((
        nested_type.clone().delimited_by(just('<'), just('>')),
        nested_type.clone().delimited_by(just('('), just(')')),
        nested_type.clone().delimited_by(just('['), just(']')),
        nested_type.delimited_by(just('{'), just('}')),
        any()
            .filter(|ch: &char| !ch.is_whitespace() && !"$`<>()[]{}".contains(*ch))
            .ignored(),
    ))
    .repeated()
    .at_least(1)
    .to_slice();
    let annotation = just(':')
        .then_ignore(just(' ').repeated())
        .ignore_then(parser_type)
        .or_not();
    let binding = just('$')
        .ignore_then(choice((binding_field, binding_index)))
        .then(annotation)
        .map(|(binding, annotation)| {
            Binding(match binding {
                Index(index, _) => Index(index, annotation),
                Field(name, _) => Field(name, annotation),
            })
        })
        .boxed();

    let symbol = chumsky::primitive::one_of(",@:;{}[]()").map(Symbol).boxed();

    let dotted_ident = ident
        .then(just('.').ignore_then(ident).repeated())
        .to_slice();

    let keyword = if metadata {
        dotted_ident.map(Keyword).boxed()
    } else {
        ident.map(Keyword).boxed()
    };

    let literal = choice((symbol, keyword))
        .delimited_by(just('`'), just('`'))
        .map(Literal)
        .boxed();

    let atom = if metadata {
        choice((
            token,
            binding,
            literal,
            dotted_ident.map(|name| Literal(Keyword(name))).boxed(),
        ))
        .boxed()
    } else {
        choice((token, binding, literal)).boxed()
    };

    atom.separated_by(just(' '))
        .collect::<Vec<PatternAtom<BindingRef<'p>>>>()
}

struct PatternAtoms<'p, B: BindingState>(Vec<PatternAtom<'p, B>>);

impl<'p, B> IntoIterator for PatternAtoms<'p, B>
where
    B: BindingState,
{
    type Item = PatternAtom<'p, B>;
    type IntoIter = <Vec<Self::Item> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'p, B> PatternAtoms<'p, B>
where
    B: BindingState,
{
    fn try_new(value: Vec<PatternAtom<'p, B>>) -> eyre::Result<Self> {
        if value.is_empty() {
            Err(eyre::eyre!("pattern cannot be empty"))
        } else {
            Ok(Self(value))
        }
    }

    fn first(&self) -> &PatternAtom<'p, B> {
        // safe to unwrap by construction
        self.0.first().unwrap()
    }

    fn contains_token(&self) -> Option<&'p str> {
        self.0.iter().find_map(PatternAtom::token)
    }
}

impl<'p> PatternAtoms<'p, BindingRef<'p>> {
    fn validate_binding_style(&self) -> eyre::Result<()> {
        if self.contains_any_field_binding() && self.contains_any_index_binding() {
            Err(eyre::eyre!("cannot combine field and index bindings"))
        } else {
            Ok(())
        }
    }

    fn contains_any_field_binding(&self) -> bool {
        self.0
            .iter()
            .any(|t| matches!(t, PatternAtom::Binding(Field(_, _))))
    }

    fn contains_any_index_binding(&self) -> bool {
        self.0
            .iter()
            .any(|t| matches!(t, PatternAtom::Binding(Index(_, _))))
    }

    fn index_bindings(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().filter_map(|t| match t {
            PatternAtom::Binding(Index(i, _)) => Some(*i),
            _ => None,
        })
    }

    fn field_bindings(&self) -> impl Iterator<Item = &'p str> + '_ {
        self.0.iter().filter_map(|t| match t {
            PatternAtom::Binding(Field(f, _)) => Some(*f),
            _ => None,
        })
    }

    fn resolve<F>(self, f: F) -> eyre::Result<PatternAtoms<'p, ResolvedBinding>>
    where
        F: Fn(BindingRef<'p>) -> eyre::Result<ResolvedBinding>,
    {
        let atoms = self
            .0
            .into_iter()
            .map(|p| p.resolve(&f))
            .collect::<eyre::Result<Vec<_>>>()?;

        Ok(PatternAtoms(atoms))
    }
}

impl<'p> UnparsedPatternInfo<'p> {
    fn parse(self) -> eyre::Result<ValidatedPatternInfo<'p>> {
        let tokens = pattern_syntax_parser(matches!(self.class, SyntaxClassAttr::Metadata { .. }))
            .parse(self.pattern)
            .into_result()
            .map_err(|errors| {
                let message = errors
                    // we'll only report this first error in the pattern
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown error when parsing pattern".to_owned());

                eyre::eyre!("invalid pattern: {message}")
            })?;

        let pattern = PatternAtoms::try_new(tokens)?;

        if matches!(self.class, SyntaxClassAttr::Instruction { .. })
            && !matches!(pattern.first(), Token(_))
        {
            return Err(eyre::eyre!(
                "the first token in a pattern must be the instruction name preceded by a tick (\"'\")"
            ));
        }

        if !matches!(self.class, SyntaxClassAttr::Instruction { .. }) {
            if let Some(tok) = pattern.contains_token() {
                return Err(eyre::eyre!(
                    "cannot have instruction syntax '{tok} in {} pattern",
                    self.class.to_string()
                ));
            }
        }

        pattern.validate_binding_style()?;

        let parsed = ParsedPatternInfo {
            pattern,
            target: self.target,
            fields: &self.fields,
            span: self.span,
        };

        match &self.fields {
            Fields::Named(n) => validate_pattern_named(parsed, n),
            Fields::Unnamed(u) => validate_pattern_unnamed(parsed, u),
            Fields::Unit => validate_pattern_unit(parsed),
        }
    }
}

impl fmt::Display for SyntaxClassAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instruction { .. } => write!(f, "instruction"),
            Self::Metadata { .. } => write!(f, "metadata"),
            Self::Type => write!(f, "type"),
            Self::Value => write!(f, "value"),
        }
    }
}

fn unnamed_types(fields: &FieldsUnnamed) -> Vec<&Type> {
    fields.unnamed.iter().map(|f| &f.ty).collect()
}

fn named_types(fields: &FieldsNamed) -> HashMap<&Ident, &Type> {
    fields
        .named
        .iter()
        .map(|f| {
            (
                f.ident
                    .as_ref()
                    .expect("fields must be named at this point"),
                &f.ty,
            )
        })
        .collect()
}

fn parse_binding_type(annotation: Option<&str>, span: Span) -> eyre::Result<Option<Type>> {
    annotation
        .map(|source| {
            syn::LitStr::new(source, span)
                .parse::<Type>()
                .map_err(|error| eyre::eyre!("invalid binding parser type: {error}"))
        })
        .transpose()
}

fn validate_pattern_unnamed<'p, 'src>(
    parsed: ParsedPatternInfo<'p, 'src>,
    fields: &FieldsUnnamed,
) -> eyre::Result<ValidatedPatternInfo<'p>> {
    if parsed.pattern.contains_any_field_binding() {
        return Err(eyre::eyre!("expected index binding for tuple fields"));
    }

    let bindings: Vec<u32> = parsed.pattern.index_bindings().collect();
    let num_indices = parsed.fields.len();
    let mut has_n: Vec<bool> = vec![false; num_indices];

    for i in bindings {
        let Some(slot) = has_n.get_mut(i as usize) else {
            return Err(eyre::eyre!("index {} is out of bounds", i));
        };

        // if we already found this i, then we have two indices
        // with the same value in our pattern
        if *slot {
            return Err(eyre::eyre!("duplicate index {} in pattern", i));
        }

        *slot = true;
    }

    if let Some(false_slot) = has_n.iter().position(|&b| !b) {
        return Err(eyre::eyre!(
            "must include all indices from 0 to {}, missing {}",
            num_indices,
            false_slot
        ));
    }

    let types = unnamed_types(fields);

    let resolved = parsed.pattern.resolve(|b| match b {
        Index(index, annotation) => {
            let ty = *types
                .get(index as usize)
                .ok_or_else(|| eyre::eyre!("index {} is out of bounds", index))?;
            let binding = format_ident!("__vihaco_field_{index}");

            Ok(ResolvedBinding {
                ty: ty.clone(),
                parser_ty: parse_binding_type(annotation, parsed.span)?,
                name: None,
                binding,
                constructor_position: index as usize,
            })
        }
        Field(_, _) => Err(eyre::eyre!("expected index binding for tuple fields")),
    })?;

    Ok(ValidatedPatternInfo {
        pattern: resolved,
        target: parsed.target,
        constructor: ConstructorType::Unnamed,
    })
}

fn validate_pattern_named<'p, 'src>(
    parsed: ParsedPatternInfo<'p, 'src>,
    fields: &FieldsNamed,
) -> eyre::Result<ValidatedPatternInfo<'p>> {
    if parsed.pattern.contains_any_index_binding() {
        return Err(eyre::eyre!("expected field binding for named fields"));
    }

    let bindings: Vec<&'p str> = parsed.pattern.field_bindings().collect();

    let types = named_types(fields);

    let mut has_name: HashMap<String, bool> = types
        .keys()
        .map(|ident| (ident.unraw().to_string(), false))
        .collect();

    for binding in bindings {
        let Some(slot) = has_name.get_mut(binding) else {
            return Err(eyre::eyre!(
                "binding name \"{}\" doesn't exist in field",
                binding
            ));
        };

        if *slot {
            return Err(eyre::eyre!("duplicate binding \"{}\" in pattern", binding));
        }

        *slot = true;
    }

    if let Some((name, _)) = has_name.iter().find(|(_, b)| !*b) {
        return Err(eyre::eyre!("must include all fields, missing {name}"));
    }

    let resolved = parsed.pattern.resolve(|b| match b {
        Field(name, annotation) => {
            let (ident, ty) = types
                .iter()
                .find(|(ident, _)| ident.unraw() == name)
                .ok_or_else(|| eyre::eyre!("binding name \"{}\" doesn't exist in field", name))?;
            let binding = format_ident!("__vihaco_field_{name}");

            Ok(ResolvedBinding {
                ty: (*ty).clone(),
                parser_ty: parse_binding_type(annotation, parsed.span)?,
                name: Some((*ident).clone()),
                binding,
                constructor_position: 0,
            })
        }
        Index(_, _) => Err(eyre::eyre!("expected field binding for named fields")),
    })?;

    Ok(ValidatedPatternInfo {
        pattern: resolved,
        target: parsed.target,
        constructor: ConstructorType::Named,
    })
}

enum PatternTarget {
    Variant { ident: Ident },
    Struct { ident: Ident },
}

impl PatternTarget {
    fn ident(&self) -> &Ident {
        match self {
            PatternTarget::Variant { ident } => ident,
            PatternTarget::Struct { ident } => ident,
        }
    }
}

fn validate_pattern_unit<'p, 'src>(
    parsed: ParsedPatternInfo<'p, 'src>,
) -> eyre::Result<ValidatedPatternInfo<'p>> {
    let resolved = parsed.pattern.resolve(|_| {
        Err(eyre::eyre!(
            "unit fields cannot have patterns with bindings"
        ))
    })?;

    Ok(ValidatedPatternInfo {
        pattern: resolved,
        target: parsed.target,
        constructor: ConstructorType::Unit,
    })
}

struct UnparsedPatternInfo<'p> {
    pattern: &'p str,
    span: Span,
    class: SyntaxClassAttr,
    target: PatternTarget,
    fields: syn::Fields,
}

struct ParsedPatternInfo<'p, 'src> {
    pattern: PatternAtoms<'p, BindingRef<'p>>,
    target: PatternTarget,
    fields: &'src syn::Fields,
    span: Span,
}

struct ResolvedBinding {
    ty: Type,
    parser_ty: Option<Type>,
    name: Option<Ident>,
    binding: Ident,
    constructor_position: usize,
}

enum ConstructorType {
    Unit,
    Named,
    Unnamed,
}

struct ValidatedPatternInfo<'p> {
    pattern: PatternAtoms<'p, ResolvedBinding>,
    target: PatternTarget,
    constructor: ConstructorType,
}

enum ParserPart {
    Ignore {
        parser: TokenStream2,
        suppresses_leading_whitespace: bool,
        suppresses_trailing_whitespace: bool,
    },
    Capture {
        parser: TokenStream2,
        name: Option<Ident>,
        binding: Ident,
        constructor_position: usize,
    },
}

fn build_parser_syntax_parts(
    pattern: PatternAtoms<ResolvedBinding>,
    paths: &ParserPaths,
) -> eyre::Result<Vec<ParserPart>> {
    let ParserPaths {
        parser_root,
        chumsky,
    } = paths;
    let mut parts = vec![];

    for atom in pattern {
        match atom {
            Token(token) => {
                parts.push(ParserPart::Ignore {
                    parser: quote! {
                        #chumsky::primitive::just(#token).ignored()
                    },
                    suppresses_leading_whitespace: false,
                    suppresses_trailing_whitespace: false,
                });
            }
            Binding(ResolvedBinding {
                ty,
                parser_ty,
                name,
                binding,
                constructor_position,
            }) => {
                let parser_ty = parser_ty.unwrap_or(ty);
                let parser = paths.parser_for(&parser_ty);
                parts.push(ParserPart::Capture {
                    parser,
                    name,
                    binding,
                    constructor_position,
                });
            }
            Literal(literal) => {
                let suppresses_leading_whitespace = literal.suppresses_leading_whitespace();
                let suppresses_trailing_whitespace = literal.suppresses_trailing_whitespace();
                let flexible = matches!(
                    literal,
                    Symbol(':' | ';' | '{' | '}' | '[' | ']' | '(' | ')')
                );
                let literal = literal.to_string();
                let parser = if flexible {
                    quote! { #chumsky::primitive::just(#literal).padded_by(#parser_root::trivia()).ignored() }
                } else {
                    quote! { #chumsky::primitive::just(#literal).ignored() }
                };
                parts.push(ParserPart::Ignore {
                    parser,
                    suppresses_leading_whitespace,
                    suppresses_trailing_whitespace,
                });
            }
        }
    }

    Ok(parts)
}

impl<'p> ValidatedPatternInfo<'p> {
    fn emit(self, paths: &ParserPaths) -> eyre::Result<(Ident, TokenStream2)> {
        let chumsky = &paths.chumsky;
        let parts = build_parser_syntax_parts(self.pattern, paths)?;

        let mut chain = None::<TokenStream2>;
        let mut pattern = None::<TokenStream2>;
        let mut bindings = Vec::<Ident>::new();
        let mut previous_suppresses_trailing_whitespace = false;
        let mut constructor_fields = Vec::<(usize, Option<Ident>, Ident)>::new();

        let ws = quote! { .then_ignore(#chumsky::primitive::just(' ').repeated().at_least(1)) };

        for part in parts {
            match part {
                ParserPart::Ignore {
                    parser,
                    suppresses_leading_whitespace,
                    suppresses_trailing_whitespace,
                } => {
                    chain = Some(match chain {
                        Some(chain)
                            if !previous_suppresses_trailing_whitespace
                                && !suppresses_leading_whitespace =>
                        {
                            quote! {
                                #chain
                                    #ws
                                    .then_ignore(#parser)
                            }
                        }
                        Some(chain) => quote! { #chain.then_ignore(#parser) },
                        None => parser,
                    });

                    previous_suppresses_trailing_whitespace = suppresses_trailing_whitespace;
                }
                ParserPart::Capture {
                    parser,
                    name,
                    binding,
                    constructor_position,
                } => {
                    chain = Some(match chain {
                        Some(chain)
                            if bindings.is_empty() && !previous_suppresses_trailing_whitespace =>
                        {
                            quote! {
                                #chain
                                    #ws
                                    .ignore_then(#parser)
                            }
                        }
                        Some(chain) if bindings.is_empty() => {
                            quote! { #chain.ignore_then(#parser) }
                        }
                        Some(chain) if !previous_suppresses_trailing_whitespace => quote! {
                            #chain
                                #ws
                                .then(#parser)
                        },
                        Some(chain) => quote! { #chain.then(#parser) },
                        None => parser,
                    });

                    previous_suppresses_trailing_whitespace = false;

                    bindings.push(binding.clone());
                    constructor_fields.push((constructor_position, name, binding.clone()));

                    pattern = Some(match pattern {
                        Some(pattern) => quote! { (#pattern, #binding) },
                        None => quote! { #binding },
                    });
                }
            }
        }

        let chain = chain.ok_or_else(|| eyre::eyre!("pattern cannot be empty"))?;

        let constructor = match &self.target {
            PatternTarget::Variant { ident } => quote! { Self::#ident },
            PatternTarget::Struct { ident } => quote! { #ident },
        };

        let map = if bindings.is_empty() {
            match self.constructor {
                ConstructorType::Unit => quote! { .map(|_| #constructor) },
                ConstructorType::Unnamed => quote! { .map(|_| #constructor()) },
                ConstructorType::Named => quote! { .map(|_| #constructor {}) },
            }
        } else {
            let pattern = pattern.expect("capture pattern exists when bindings exist");
            match self.constructor {
                ConstructorType::Unit => unreachable!("unit constructors have no bindings"),
                ConstructorType::Named => {
                    let fields = constructor_fields.iter().map(|(_, name, binding)| {
                        let name = name
                            .as_ref()
                            .expect("named constructor bindings have field names");
                        quote! { #name: #binding }
                    });
                    quote! { .map(|#pattern| #constructor { #(#fields),* }) }
                }
                ConstructorType::Unnamed => {
                    constructor_fields.sort_by_key(|(position, _, _)| *position);
                    let fields = constructor_fields.iter().map(|(_, _, binding)| binding);
                    quote! { .map(|#pattern| #constructor(#(#fields),*)) }
                }
            }
        };

        let name = format_ident!(
            "__vihaco_pattern_for_{}",
            &self.target.ident().to_string().to_lowercase()
        );

        Ok((
            name.clone(),
            quote! {
                let #name = #chain #map;
            },
        ))
    }
}

struct PatternCompilationInfo<'src> {
    fields: &'src Fields,
    pattern_info: Option<PatternInfo>,
    target: PatternTarget,
    class: Option<SyntaxClassAttr>,
}

impl<'src> PatternCompilationInfo<'src> {
    fn with_new_info(self, pattern: String, span: Span) -> PatternCompilationInfo<'src> {
        Self {
            fields: self.fields,
            pattern_info: Some(PatternInfo(pattern, span)),
            target: self.target,
            class: self.class,
        }
    }
}

fn num_fields(f: &syn::Fields) -> usize {
    match f {
        Fields::Named(f) => f.named.iter().len(),
        Fields::Unnamed(f) => f.unnamed.iter().len(),
        Fields::Unit => 0,
    }
}

fn generate_pattern<'src>(
    info: PatternCompilationInfo<'src>,
    span: Span,
) -> eyre::Result<PatternCompilationInfo<'src>> {
    if matches!(info.class, Some(SyntaxClassAttr::Type)) {
        return Err(eyre::eyre!("types must provide patterns"));
    }

    let name = info.target.ident().to_string().to_lowercase();
    let prefix = if matches!(info.class, Some(SyntaxClassAttr::Instruction { .. })) {
        Some(format!("'{}", name))
    } else {
        None
    };

    if matches!(info.class, Some(SyntaxClassAttr::Value)) {
        let size = num_fields(info.fields);
        if size > 1 {
            return Err(eyre::eyre!(
                "values without patterns must have at most one field"
            ));
        }

        if size == 0 {
            let pattern = format!("`{name}`");
            return Ok(info.with_new_info(pattern, span));
        }
    }

    Ok(match info.fields {
        Fields::Named(f) => {
            let pattern = f
                .named
                .iter()
                .map(|f| format!("${}", f.ident.as_ref().expect("they're named").unraw()))
                .collect::<Vec<_>>()
                .join(" `,` ");

            let pattern = if let Some(prefix) = prefix {
                format!("{} {}", prefix, pattern)
            } else {
                pattern
            };
            info.with_new_info(pattern, span)
        }
        Fields::Unnamed(f) => {
            let len = f.unnamed.iter().len();

            let pattern = (0..len)
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(" `,` ");

            let pattern = if let Some(prefix) = prefix {
                format!("{} {}", prefix, pattern)
            } else {
                pattern
            };
            info.with_new_info(pattern, span)
        }
        Fields::Unit => info.with_new_info(prefix.unwrap_or(name), span),
    })
}

type InstructionWithSpan = (String, Span);

fn compile_pattern_parser(
    info: PatternCompilationInfo,
    decl_span: Span,
    variant_span: Span,
    paths: &ParserPaths,
) -> Result<(Ident, TokenStream2, Option<InstructionWithSpan>)> {
    let Some(ref class) = info.class else {
        return Err(Error::new(
            decl_span,
            "#[pattern] requires a #[syntax_class] on the enum definition",
        ));
    };

    if let Some(PatternInfo(pattern, span)) = info.pattern_info {
        let info = UnparsedPatternInfo {
            pattern: pattern.as_str(),
            span,
            class: class.clone(),
            target: info.target,
            fields: info.fields.clone(),
        };

        let validated = info
            .parse()
            .map_err(|err| Error::new(span, err.to_string()))?;
        let instruction_token = validated
            .pattern
            .contains_token()
            .map(|token| (token.to_owned(), span));
        let (ident, parser) = validated
            .emit(paths)
            .map_err(|err| Error::new(span, err.to_string()))?;
        Ok((ident, parser, instruction_token))
    } else {
        let new_info = generate_pattern(info, variant_span)
            .map_err(|err| Error::new(variant_span, err.to_string()))?;
        compile_pattern_parser(new_info, decl_span, variant_span, paths)
    }
}

/// Combine alternatives without the deep types produced by chained `.or()`.
/// Chumsky supports tuple choices up to 26 elements; group larger choices.
fn emit_choice(alternatives: &[TokenStream2], chumsky: &syn::Path) -> TokenStream2 {
    match alternatives {
        [] => unreachable!("empty choices are handled by the caller"),
        [only] => only.clone(),
        alternatives if alternatives.len() <= 26 => {
            quote!(#chumsky::primitive::choice((#(#alternatives),*)))
        }
        alternatives => {
            let groups = alternatives
                .chunks(26)
                .map(|group| emit_choice(group, chumsky))
                .collect::<Vec<_>>();
            emit_choice(&groups, chumsky)
        }
    }
}

fn apply_head(
    parser: TokenStream2,
    class: Option<&SyntaxClassAttr>,
    paths: &ParserPaths,
) -> TokenStream2 {
    let chumsky = &paths.chumsky;
    match class {
        Some(SyntaxClassAttr::Instruction { head: Some(head) }) => {
            let head = format!("{head}.");
            quote!(#chumsky::primitive::just(#head).ignore_then(#parser))
        }
        Some(SyntaxClassAttr::Metadata {
            head: Some(head),
            delimiters: Some(_),
        }) => {
            quote!(#chumsky::primitive::just(#head)
                .then_ignore(#chumsky::text::whitespace().at_least(1))
                .ignore_then(#parser))
        }
        Some(SyntaxClassAttr::Metadata {
            head: Some(head), ..
        }) => {
            let head = format!("{head} ");
            quote!(#chumsky::primitive::just(#head).ignore_then(#parser))
        }
        _ => parser,
    }
}

fn emit_parse_impl(
    ident: &Ident,
    generics: &Generics,
    src_lifetime: &Lifetime,
    body: TokenStream2,
    paths: &ParserPaths,
) -> TokenStream2 {
    let ParserPaths {
        parser_root,
        chumsky,
    } = paths;
    let parser_generics = generics_with_lifetime(generics, src_lifetime);
    let (impl_generics, _, where_clause) = parser_generics.split_for_impl();
    let (_, ty_generics, _) = generics.split_for_impl();
    quote! {
        impl #impl_generics #parser_root::Parse<#src_lifetime> for #ident #ty_generics #where_clause {
            fn parser() -> impl #chumsky::Parser<
                #src_lifetime,
                &#src_lifetime str,
                Self,
                #chumsky::extra::Err<#chumsky::error::Simple<#src_lifetime, char>>,
            > {
                use #chumsky::Parser as _;
                #body
            }
        }
    }
}

fn expand_enum(input: EnumInfo, paths: &ParserPaths) -> Result<TokenStream> {
    let ParserPaths {
        parser_root,
        chumsky,
    } = paths;
    let enum_ident = &input.ident;
    let enum_attrs = EnumAttrs::from_attrs(input.attrs)?;
    if enum_attrs.syntax_class.is_none() {
        if let Some(pattern) = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("pattern"))
        {
            return Err(Error::new(
                pattern.span(),
                "#[pattern] requires a #[syntax_class] on the variant definition",
            ));
        }
        return Err(Error::new_spanned(
            enum_ident,
            "#[derive(Parse)] on an enum requires a #[syntax_class(...)] attribute",
        ));
    };

    if matches!(
        enum_attrs.syntax_class,
        Some(SyntaxClassAttr::Metadata {
            delimiters: Some(_),
            ..
        })
    ) {
        return Err(Error::new_spanned(
            enum_ident,
            "metadata delimiters require a named struct",
        ));
    }

    let src_lifetime = fresh_lifetime(input.generics, "__vihaco_src");

    let data = input.data;

    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            enum_ident,
            "#[derive(Parse)] requires at least one variant",
        ));
    }

    // Parse all variant attrs + compute tokens
    let mut variant_data: Vec<(Ident, TokenStream2, Option<InstructionWithSpan>)> = vec![];
    let mut instruction_names = HashSet::new();
    for variant in &data.variants {
        let vattrs = VariantAttrs::from_variant(variant)?;

        let pattern_compilation_info = PatternCompilationInfo {
            fields: &variant.fields,
            pattern_info: vattrs.pattern,
            target: PatternTarget::Variant {
                ident: variant.ident.clone(),
            },
            class: enum_attrs.syntax_class.clone(),
        };

        let ident_and_parser = compile_pattern_parser(
            pattern_compilation_info,
            enum_ident.span(),
            variant.span(),
            paths,
        )?;

        if let Some((token, span)) = &ident_and_parser.2 {
            if !instruction_names.insert(token.clone()) {
                return Err(Error::new(
                    *span,
                    format!("duplicate instruction name `{token}`"),
                ));
            }
        }

        variant_data.push(ident_and_parser);
    }

    // A parser such as `just("v2")` succeeds on the prefix of `v25`. Since
    // end-of-input (or a list separator) lives outside the instruction parser,
    // shorter alternatives must not shadow longer instruction tokens.
    variant_data.sort_by_key(|(_, _, token)| {
        std::cmp::Reverse(token.as_ref().map_or(0, |(token, _)| token.len()))
    });

    let alternatives = variant_data
        .iter()
        .map(|(name, _, _)| quote!(#name))
        .collect::<Vec<_>>();
    let parser = apply_head(
        emit_choice(&alternatives, chumsky),
        enum_attrs.syntax_class.as_ref(),
        paths,
    );
    let variant_bindings = variant_data.iter().map(|(_, binding, _)| binding);
    let parse_impl = emit_parse_impl(
        enum_ident,
        input.generics,
        &src_lifetime,
        quote! { #(#variant_bindings)* #parser },
        paths,
    );

    let surface_instruction_impl = if matches!(
        &enum_attrs.syntax_class,
        Some(SyntaxClassAttr::Instruction { .. })
    ) {
        let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
        quote! {
            impl #impl_generics #parser_root::SurfaceInstruction for #enum_ident #ty_generics #where_clause {}
        }
    } else {
        quote! {}
    };

    let output = quote! { #parse_impl #surface_instruction_impl };

    Ok(output.into())
}

struct ExpansionInfo<'src, T> {
    data: &'src T,
    ident: &'src Ident,
    attrs: &'src [Attribute],
    generics: &'src Generics,
}

type EnumInfo<'src> = ExpansionInfo<'src, syn::DataEnum>;
type StructInfo<'src> = ExpansionInfo<'src, syn::DataStruct>;

fn expand_struct(input: StructInfo, paths: &ParserPaths) -> Result<TokenStream> {
    compile_struct(input, paths).map(|(parser, _)| parser)
}

fn compile_struct(
    input: StructInfo,
    paths: &ParserPaths,
) -> Result<(TokenStream, Option<InstructionWithSpan>)> {
    let struct_ident = input.ident;
    let struct_attrs = StructAttrs::from_attrs(input.attrs)?;
    if struct_attrs.syntax_class.is_none() {
        if let Some(pattern) = input
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("pattern"))
        {
            return Err(Error::new(
                pattern.span(),
                "#[pattern] requires a #[syntax_class] on the struct definition",
            ));
        }
        return Err(Error::new_spanned(
            struct_ident,
            "#[derive(Parse)] on a struct requires a #[syntax_class(...)] attribute",
        ));
    }

    let src_lifetime = fresh_lifetime(input.generics, "__vihaco_src");
    let mut generics = input.generics.clone();
    let (body, mnemonic) = if let Some(SyntaxClassAttr::Metadata {
        delimiters: Some((open, close)),
        ..
    }) = &struct_attrs.syntax_class
    {
        if struct_attrs.pattern.is_some() {
            return Err(Error::new_spanned(
                struct_ident,
                "delimited metadata generates its fields; omit #[pattern]",
            ));
        }
        let body = emit_metadata_record(&input, paths, &src_lifetime, open, close)?;
        for field in &input.data.fields {
            let ty = &field.ty;
            let parser_root = &paths.parser_root;
            generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote!(#ty: #parser_root::Parse<#src_lifetime> + #src_lifetime));
        }
        (body, None)
    } else {
        let info = PatternCompilationInfo {
            fields: &input.data.fields,
            pattern_info: struct_attrs.pattern,
            target: PatternTarget::Struct {
                ident: struct_ident.clone(),
            },
            class: struct_attrs.syntax_class.clone(),
        };
        let span = struct_ident.span();
        let (ident, binding, mnemonic) = compile_pattern_parser(info, span, span, paths)?;
        (quote! { { #binding #ident } }, mnemonic)
    };
    let parser = apply_head(body, struct_attrs.syntax_class.as_ref(), paths);
    let output = emit_parse_impl(struct_ident, &generics, &src_lifetime, parser, paths);
    Ok((output.into(), mnemonic))
}

/// Emit the unordered record body; struct emission supplies its head and impl.
fn emit_metadata_record(
    input: &StructInfo<'_>,
    paths: &ParserPaths,
    lifetime: &Lifetime,
    open: &str,
    close: &str,
) -> Result<TokenStream2> {
    let Fields::Named(fields) = &input.data.fields else {
        return Err(Error::new_spanned(
            input.ident,
            "metadata delimiters require a named struct",
        ));
    };
    let ParserPaths {
        parser_root,
        chumsky,
    } = paths;
    let names: Vec<_> = fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect();
    let types: Vec<_> = fields.named.iter().map(|field| &field.ty).collect();
    let mut used: HashSet<_> = input
        .generics
        .const_params()
        .map(|param| param.ident.to_string())
        .collect();
    let mut fresh = |base: &str| {
        let mut candidate = base.to_owned();
        while !used.insert(candidate.clone()) {
            candidate.push('_');
        }
        Ident::new(&candidate, Span::mixed_site())
    };
    let entries_binding = fresh("__entries");
    let span_binding = fresh("__span");
    let value_binding = fresh("__value");
    let slots: Vec<_> = (0..names.len())
        .map(|i| fresh(&format!("__slot_{i}")))
        .collect();
    let incoming: Vec<_> = (0..names.len())
        .map(|i| fresh(&format!("__incoming_{i}")))
        .collect();
    let alternatives: Vec<_> = names
        .iter()
        .zip(&types)
        .enumerate()
        .map(|(index, (name, ty))| {
            let key = name.unraw().to_string();
            let value_parser = paths.parser_for(ty);
            let values = types.iter().enumerate().map(|(position, ty)| {
                if index == position {
                    quote!(::core::option::Option::<#ty>::Some(#value_binding))
                } else {
                    quote!(::core::option::Option::<#ty>::None)
                }
            });
            quote! {
                #parser_root::metadata_key(#key)
                    .then_ignore(#chumsky::primitive::just(':').padded_by(#parser_root::trivia()))
                    .ignore_then((#value_parser).padded_by(#parser_root::trivia()))
                    .map(|#value_binding| (#(#values,)*))
            }
        })
        .collect();
    let body = if alternatives.is_empty() {
        // Empty named records still parse `{}` and reject any entries.
        quote!(#chumsky::primitive::any().try_map(|_, #span_binding| -> ::core::result::Result<(), #chumsky::error::Simple<#lifetime, char>> {
            Err(#chumsky::error::Simple::new(None, #span_binding))
        }))
    } else {
        emit_choice(&alternatives, chumsky)
    };
    Ok(quote! {{
        use #chumsky::IterParser as _;
        (#body).separated_by(#chumsky::primitive::just(',').padded_by(#parser_root::trivia()))
            .allow_trailing().collect::<::std::vec::Vec<_>>()
            .delimited_by(
                #chumsky::primitive::just(#open).padded_by(#parser_root::trivia()),
                #chumsky::primitive::just(#close).padded_by(#parser_root::trivia()),
            ).try_map(|#entries_binding, #span_binding| {
                    #(let mut #slots: ::core::option::Option<#types> = None;)*
                    for (#(#incoming,)*) in #entries_binding {
                        #(if let Some(#value_binding) = #incoming {
                            if #slots.replace(#value_binding).is_some() {
                                return Err(#chumsky::error::Simple::new(None, #span_binding));
                            }
                        })*
                    }
                    Ok(Self {
                        #(#names: #slots.ok_or_else(|| #chumsky::error::Simple::new(None, #span_binding))?,)*
                    })
        })
    }})
}

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    expand_rooted(
        input,
        &ParserPaths {
            parser_root: syn::parse_quote!(::vihaco_parser),
            chumsky: syn::parse_quote!(::chumsky),
        },
    )
}

fn expand_rooted(input: DeriveInput, paths: &ParserPaths) -> Result<TokenStream> {
    match &input.data {
        Data::Enum(e) => expand_enum(
            EnumInfo {
                data: e,
                ident: &input.ident,
                attrs: &input.attrs,
                generics: &input.generics,
            },
            paths,
        ),
        Data::Struct(s) => expand_struct(
            StructInfo {
                data: s,
                ident: &input.ident,
                attrs: &input.attrs,
                generics: &input.generics,
            },
            paths,
        ),
        _ => Err(Error::new_spanned(
            &input,
            "#[derive(Parse)] is not supported on unions",
        )),
    }
}

fn generics_with_lifetime(generics: &syn::Generics, src_lifetime: &Lifetime) -> syn::Generics {
    let mut generics = generics.clone();
    generics.params.insert(
        0,
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(src_lifetime.clone())),
    );

    generics
}

fn fresh_lifetime(generics: &syn::Generics, base: &str) -> Lifetime {
    let used: BTreeSet<String> = generics
        .lifetimes()
        .map(|param| param.lifetime.ident.to_string())
        .collect();

    if !used.contains(base) {
        return Lifetime::new(&format!("'{base}"), proc_macro2::Span::call_site());
    }

    for suffix in 0usize.. {
        let candidate = format!("{base}_{suffix}");
        if !used.contains(&candidate) {
            return Lifetime::new(&format!("'{candidate}"), proc_macro2::Span::call_site());
        }
    }

    unreachable!("unbounded fresh lifetime search")
}

// Internal emitter hook: the dialect owns declarations; the parser owns pattern
// validation, including the mnemonic used by the typed association.
pub fn dialect_parsers(root: TokenStream2, item: TokenStream2) -> Result<TokenStream2> {
    let root: syn::Path = syn::parse2(root)?;
    let mut module: syn::ItemMod = syn::parse2(item)?;
    let items = &mut module
        .content
        .as_mut()
        .ok_or_else(|| Error::new(module.ident.span(), "expected inline dialect module"))?
        .1;
    let mut leaves = Vec::new();
    for (position, item) in items.iter_mut().enumerate() {
        if let syn::Item::Struct(instruction) = item {
            if let Some(index) = instruction
                .attrs
                .iter()
                .position(|attr| attr.path().is_ident("__vihaco_runtime"))
            {
                let runtime: Ident = instruction.attrs.remove(index).parse_args()?;
                leaves.push((position, runtime, instruction.clone()));
            }
        }
    }
    let paths = ParserPaths {
        parser_root: root.clone(),
        chumsky: syn::parse_quote!(#root::chumsky),
    };
    let mut emitted = Vec::new();
    let mut mnemonics = HashSet::new();
    let mut patterns = HashSet::new();
    // Validate every parser first so a collision cannot mask malformed syntax.
    let mut validated = Vec::new();
    for (position, runtime, mut declaration) in leaves {
        let attrs = StructAttrs::from_attrs(&declaration.attrs)?;
        let pattern = attrs.pattern.as_ref().map(|p| p.0.clone());
        let syntax_types = dialect_syntax_types(&declaration, &attrs)?;
        let mut syntax = declaration.clone();
        for (field, ty) in syntax.fields.iter_mut().zip(&syntax_types) {
            field.ty = ty.clone();
        }
        let input: DeriveInput = syn::parse2(quote!(#syntax))?;
        let data = match &input.data {
            Data::Struct(data) => data,
            _ => unreachable!("dialect leaf is a struct"),
        };
        let (parser, mnemonic) = compile_struct(
            StructInfo {
                data,
                ident: &input.ident,
                attrs: &input.attrs,
                generics: &input.generics,
            },
            &paths,
        )?;
        let differing: Vec<_> = declaration
            .fields
            .iter()
            .zip(&syntax_types)
            .enumerate()
            .filter(|(_, (field, ty))| field.ty != **ty)
            .map(|(index, (field, ty))| (index, field.ty.clone(), ty.clone()))
            .collect();
        let syntax_ty = if differing.is_empty() {
            quote!(#runtime)
        } else {
            quote!(#runtime<#root::Syntax>)
        };
        let mut parser: syn::File = syn::parse2(TokenStream2::from(parser))?;
        for item in &mut parser.items {
            if let syn::Item::Impl(implementation) = item {
                *implementation.self_ty = syn::parse2(syntax_ty.clone())?;
            }
        }
        declaration
            .attrs
            .retain(|a| !a.path().is_ident("syntax_class") && !a.path().is_ident("pattern"));
        let loader = fresh_generated_ident("__VihacoLoader", quote!(#root #declaration));
        let support = if differing.is_empty() {
            quote! {
                impl<#loader: ?Sized> #root::Lower<#loader> for #runtime {
                    type Runtime = Self;
                    fn lower(self, _: &mut #loader) -> #root::eyre::Result<Self> {
                        Ok(self)
                    }
                }
            }
        } else {
            let types = format_ident!("__{}Types", runtime);
            let params: Vec<_> = differing
                .iter()
                .map(|(index, _, _)| format_ident!("Param{index}"))
                .collect();
            let runtime_types: Vec<_> = differing.iter().map(|(_, ty, _)| ty).collect();
            let source_types: Vec<_> = differing.iter().map(|(_, _, ty)| ty).collect();
            // The phase selector must not capture an authored operand type.
            let phase = fresh_generated_ident(
                &format!("__Vihaco{runtime}Phase"),
                quote!(#root #declaration),
            );
            declaration.generics = syn::parse_quote!(<#phase: #types + ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq = #root::Runtime>);
            for ((index, _, _), param) in differing.iter().zip(&params) {
                declaration
                    .fields
                    .iter_mut()
                    .nth(*index)
                    .expect("validated field")
                    .ty = syn::parse_quote!(<#phase as #types>::#param);
            }
            quote! {
                #[doc(hidden)]
                pub trait #types {
                    #(type #params: ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::PartialEq;)*
                }
                impl #types for #root::Syntax {
                    #(type #params = #source_types;)*
                }
                impl #types for #root::Runtime {
                    #(type #params = #runtime_types;)*
                }
            }
        };
        items[position] = syn::Item::Struct(declaration);
        validated.push((runtime, syntax_ty, parser, support, mnemonic, pattern));
    }
    for (runtime, syntax_ty, parser, support, mnemonic, pattern) in validated {
        let (mnemonic, span) =
            mnemonic.ok_or_else(|| Error::new(runtime.span(), "expected instruction mnemonic"))?;
        if let Some(pattern) = pattern {
            if !patterns.insert(pattern) {
                return Err(Error::new(span, "duplicate instruction pattern"));
            }
        }
        if !mnemonics.insert(mnemonic.clone()) {
            return Err(Error::new(
                span,
                format!("duplicate instruction name `{mnemonic}`"),
            ));
        }
        emitted.push(quote! {
            #parser
            #support
            impl #root::__private::InstructionSpec for #runtime {
                type Syntax = #syntax_ty;
                type Dialect = __VihacoDialect;
                const MNEMONIC: &'static str = #mnemonic;
            }
        });
    }
    let generated: syn::File = syn::parse2(quote!(#(#emitted)*))?;
    items.extend(generated.items);
    Ok(quote!(#module))
}

/// Resolve retained operand types using the same validated pattern grammar as parsing.
fn dialect_syntax_types(instruction: &syn::ItemStruct, attrs: &StructAttrs) -> Result<Vec<Type>> {
    let mut types: Vec<_> = instruction
        .fields
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    if let Some(PatternInfo(pattern, span)) = &attrs.pattern {
        let validated = UnparsedPatternInfo {
            pattern,
            span: *span,
            class: SyntaxClassAttr::Instruction { head: None },
            target: PatternTarget::Struct {
                ident: instruction.ident.clone(),
            },
            fields: instruction.fields.clone(),
        }
        .parse()
        .map_err(|error| Error::new(*span, error.to_string()))?;
        for atom in validated.pattern {
            if let Binding(binding) = atom {
                if let Some(ty) = binding.parser_ty {
                    types[binding.constructor_position] = ty;
                }
            }
        }
    }
    Ok(types)
}

/// Avoid capturing authored type names or framework paths with generated parameters.
fn fresh_generated_ident(base: &str, tokens: TokenStream2) -> Ident {
    fn collect(tokens: TokenStream2, names: &mut HashSet<String>) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(ident) => {
                    names.insert(ident.to_string());
                }
                proc_macro2::TokenTree::Group(group) => collect(group.stream(), names),
                _ => {}
            }
        }
    }
    let mut names = HashSet::new();
    collect(tokens, &mut names);
    let mut candidate = base.to_owned();
    while names.contains(&candidate) {
        candidate.push('_');
    }
    Ident::new(&candidate, Span::call_site())
}
