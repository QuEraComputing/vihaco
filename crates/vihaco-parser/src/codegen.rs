// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{
    attr::{EnumAttrs, FieldAttrs, PatternInfo, SyntaxClassAttr, VariantAttrs},
    codegen::{
        BindingRef::{Field, Index},
        PatternAtom::{Binding, Literal, Token},
        PatternLiteral::{Keyword, Symbol},
    },
};
use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra,
    primitive::{choice, just},
    text,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::{
    collections::{BTreeSet, HashMap},
    fmt::{self, Display},
    vec,
};
use syn::{
    Attribute, Data, DeriveInput, Error, Fields, FieldsNamed, FieldsUnnamed, Generics, Ident,
    Lifetime, Result, Type, spanned::Spanned,
};

#[derive(PartialEq)]
enum BindingRef<'p> {
    Index(u32),
    Field(&'p str),
}

#[derive(PartialEq)]
enum PatternLiteral<'p> {
    Keyword(&'p str),
    Symbol(char),
}

impl PatternLiteral<'_> {
    fn suppresses_leading_whitespace(&self) -> bool {
        matches!(self, Symbol(','))
    }

    fn suppresses_trailing_whitespace(&self) -> bool {
        matches!(self, Symbol('@'))
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

fn pattern_syntax_parser<'p>()
-> impl Parser<'p, &'p str, Vec<PatternAtom<'p, BindingRef<'p>>>, extra::Err<Rich<'p, char>>> {
    let ident = text::ascii::ident();
    let digits = text::int(10);

    let token = just('\'').ignore_then(ident).map(Token);

    let binding_index = digits
        .to_slice()
        .try_map(|s: &str, span| {
            s.parse::<u32>()
                .map_err(|_| Rich::custom(span, "binding index must be a valid number"))
        })
        .map(Index);

    let binding_field = ident.map(Field);

    let binding = just('$')
        .ignore_then(choice((binding_field, binding_index)))
        .map(Binding);

    let symbol = choice((just(','), just('@'))).map(Symbol);

    let keyword = ident.map(Keyword);

    let literal = choice((symbol, keyword))
        .delimited_by(just('`'), just('`'))
        .map(Literal);

    choice((token, binding, literal))
        .separated_by(just(' '))
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
            .any(|t| matches!(t, PatternAtom::Binding(Field(_))))
    }

    fn contains_any_index_binding(&self) -> bool {
        self.0
            .iter()
            .any(|t| matches!(t, PatternAtom::Binding(Index(_))))
    }

    fn index_bindings(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().filter_map(|t| match t {
            PatternAtom::Binding(Index(i)) => Some(*i),
            _ => None,
        })
    }

    fn field_bindings(&self) -> impl Iterator<Item = &'p str> + '_ {
        self.0.iter().filter_map(|t| match t {
            PatternAtom::Binding(Field(f)) => Some(*f),
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

impl<'p, F> UnparsedPatternInfo<'p, F>
where
    F: FieldShape,
{
    fn parse(self) -> eyre::Result<ValidatedPatternInfo<'p>> {
        let tokens = pattern_syntax_parser()
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

        if matches!(self.class, SyntaxClassAttr::Instruction)
            && !matches!(pattern.first(), Token(_))
        {
            return Err(eyre::eyre!(
                "the first token in a pattern must be the instruction name preceded by a tick (\"'\")"
            ));
        }

        if !matches!(self.class, SyntaxClassAttr::Instruction)
            && let Some(tok) = pattern.contains_token()
        {
            return Err(eyre::eyre!(
                "cannot have instruction syntax '{tok} in {} pattern",
                self.class.to_string()
            ));
        }

        pattern.validate_binding_style()?;

        let parsed = ParsedPatternInfo {
            pattern,
            target: self.target,
            fields: self.fields,
        };

        F::validate_pattern(parsed)
    }
}

impl fmt::Display for SyntaxClassAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instruction => write!(f, "instruction"),
            Self::Type => write!(f, "type"),
            Self::Value => write!(f, "value"),
        }
    }
}

impl FieldShape for UnnamedFields<'_> {
    fn validate_pattern<'p>(
        parsed: ParsedPatternInfo<'p, Self>,
    ) -> eyre::Result<ValidatedPatternInfo<'p>> {
        let bindings: Vec<u32> = parsed.pattern.index_bindings().collect();
        let num_indices = parsed.fields.count;
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

        let resolved = parsed.pattern.resolve(|b| match b {
            Index(index) => {
                let ty = *parsed
                    .fields
                    .types
                    .get(index as usize)
                    .ok_or_else(|| eyre::eyre!("index {} is out of bounds", index))?;
                let binding = format_ident!("__vihaco_field_{index}");

                Ok(ResolvedBinding {
                    ty: ty.clone(),
                    name: None,
                    binding,
                })
            }
            Field(_) => Err(eyre::eyre!("expected index binding for tuple fields")),
        })?;

        Ok(ValidatedPatternInfo {
            pattern: resolved,
            target: parsed.target,
            constructor: ConstructorType::Unnamed,
        })
    }
}

impl FieldShape for NamedFields<'_> {
    fn validate_pattern<'p>(
        parsed: ParsedPatternInfo<'p, Self>,
    ) -> eyre::Result<ValidatedPatternInfo<'p>> {
        let bindings: Vec<&'p str> = parsed.pattern.field_bindings().collect();

        let mut has_name: HashMap<String, bool> = parsed
            .fields
            .types
            .keys()
            .map(|t| (t.to_string(), false))
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
            Field(name) => {
                let ident = Ident::new(name, proc_macro2::Span::call_site());
                let ty = *parsed.fields.types.get(&ident).ok_or_else(|| {
                    eyre::eyre!("binding name \"{}\" doesn't exist in field", name)
                })?;
                let binding = format_ident!("__vihaco_field_{name}");

                Ok(ResolvedBinding {
                    ty: ty.clone(),
                    name: Some(ident.clone()),
                    binding,
                })
            }
            Index(_) => Err(eyre::eyre!("expected field binding for named fields")),
        })?;

        Ok(ValidatedPatternInfo {
            pattern: resolved,
            target: parsed.target,
            constructor: ConstructorType::Named,
        })
    }
}

enum PatternTarget {
    Variant { ident: Ident },
}

impl PatternTarget {
    fn ident(&self) -> &Ident {
        match self {
            PatternTarget::Variant { ident } => ident,
        }
    }
}

struct UnnamedFields<'src> {
    count: usize,
    types: Vec<&'src Type>,
}

impl<'src> From<&'src FieldsUnnamed> for UnnamedFields<'src> {
    fn from(value: &'src FieldsUnnamed) -> Self {
        let types: Vec<&'src Type> = value.unnamed.iter().map(|f| &f.ty).collect();

        Self {
            count: types.len(),
            types,
        }
    }
}

struct NamedFields<'src> {
    types: HashMap<&'src Ident, &'src Type>,
}

#[derive(Default)]
struct UnitFields {}

impl FieldShape for UnitFields {
    fn validate_pattern<'p>(
        parsed: ParsedPatternInfo<'p, Self>,
    ) -> eyre::Result<ValidatedPatternInfo<'p>> {
        let resolved = parsed.pattern.resolve(|_| {
            Err(eyre::eyre!(
                "unit fields cannot have patterns with bindings"
            ))
        })?;

        Ok(ValidatedPatternInfo {
            pattern: resolved,
            target: parsed.target,
            constructor: ConstructorType::Unnamed,
        })
    }
}

impl<'src> From<&'src FieldsNamed> for NamedFields<'src> {
    fn from(value: &'src FieldsNamed) -> Self {
        let types: HashMap<&'src Ident, &'src Type> = value
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
            .collect();

        Self { types }
    }
}

trait FieldShape: Sized {
    fn validate_pattern<'p>(
        parsed: ParsedPatternInfo<'p, Self>,
    ) -> eyre::Result<ValidatedPatternInfo<'p>>;
}

struct UnparsedPatternInfo<'p, Info: FieldShape> {
    pattern: &'p str,
    class: SyntaxClassAttr,
    target: PatternTarget,
    fields: Info,
}

struct ParsedPatternInfo<'p, Info: FieldShape> {
    pattern: PatternAtoms<'p, BindingRef<'p>>,
    target: PatternTarget,
    fields: Info,
}

struct ResolvedBinding {
    ty: Type,
    name: Option<Ident>,
    binding: Ident,
}

enum ConstructorType {
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
    },
}

fn build_parser_syntax_parts(
    pattern: PatternAtoms<ResolvedBinding>,
) -> eyre::Result<Vec<ParserPart>> {
    let mut parts = vec![];

    for atom in pattern {
        match atom {
            Token(token) => {
                parts.push(ParserPart::Ignore {
                    parser: quote! {
                        ::chumsky::primitive::just(#token).ignored()
                    },
                    suppresses_leading_whitespace: false,
                    suppresses_trailing_whitespace: false,
                });
            }
            Binding(ResolvedBinding { ty, name, binding }) => {
                parts.push(ParserPart::Capture {
                    parser: quote! {
                        <#ty as ::vihaco_parser_core::Parse>::parser()
                    },
                    name,
                    binding,
                });
            }
            Literal(literal) => {
                let suppresses_leading_whitespace = literal.suppresses_leading_whitespace();
                let suppresses_trailing_whitespace = literal.suppresses_trailing_whitespace();
                let literal = literal.to_string();
                parts.push(ParserPart::Ignore {
                    parser: quote! {
                        ::chumsky::primitive::just(#literal).ignored()
                    },
                    suppresses_leading_whitespace,
                    suppresses_trailing_whitespace,
                });
            }
        }
    }

    Ok(parts)
}

impl<'p> ValidatedPatternInfo<'p> {
    fn emit(self) -> eyre::Result<(Ident, TokenStream2)> {
        let parts = build_parser_syntax_parts(self.pattern)?;

        let mut chain = None::<TokenStream2>;
        let mut pattern = None::<TokenStream2>;
        let mut bindings = Vec::<Ident>::new();
        let mut previous_suppresses_trailing_whitespace = false;
        let mut constructor_fields = Vec::<(Option<Ident>, Ident)>::new();

        let ws = quote! { .then_ignore(::chumsky::primitive::just(' ').repeated().at_least(1)) };

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
                    constructor_fields.push((name, binding.clone()));

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
        };

        let map = if bindings.is_empty() {
            quote! { .map(|_| #constructor) }
        } else {
            let pattern = pattern.expect("capture pattern exists when bindings exist");
            match self.constructor {
                ConstructorType::Named => {
                    let fields = constructor_fields.iter().map(|(name, binding)| {
                        let name = name
                            .as_ref()
                            .expect("named constructor bindings have field names");
                        quote! { #name: #binding }
                    });
                    quote! { .map(|#pattern| #constructor { #(#fields),* }) }
                }
                ConstructorType::Unnamed => {
                    quote! { .map(|#pattern| #constructor(#(#bindings),*)) }
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
    let prefix = if matches!(info.class, Some(SyntaxClassAttr::Instruction)) {
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
                .map(|f| format!("${}", f.ident.as_ref().expect("they're named")))
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
        Fields::Unit => info.with_new_info(
            prefix.expect("must be an instruction with a prefix at this point in the execution"),
            span,
        ),
    })
}

fn compile_pattern_parser(
    info: PatternCompilationInfo,
    decl_span: Span,
    variant_span: Span,
) -> Result<(Ident, TokenStream2)> {
    let Some(class) = info.class else {
        return Err(Error::new(
            decl_span,
            "#[pattern] requires a #[syntax_class] on the enum definition",
        ));
    };

    if let Some(PatternInfo(pattern, span)) = info.pattern_info {
        // TODO: use dynamic dispatch
        match &info.fields {
            Fields::Named(n) => {
                let fields = NamedFields::from(n);
                let info = UnparsedPatternInfo {
                    pattern: pattern.as_str(),
                    class,
                    target: info.target,
                    fields,
                };

                info.parse()
                    .and_then(ValidatedPatternInfo::emit)
                    .map_err(|err| Error::new(span, err.to_string()))
            }

            Fields::Unnamed(u) => {
                let fields = UnnamedFields::from(u);
                let info = UnparsedPatternInfo {
                    pattern: pattern.as_str(),
                    class,
                    target: info.target,
                    fields,
                };

                info.parse()
                    .and_then(ValidatedPatternInfo::emit)
                    .map_err(|err| Error::new(span, err.to_string()))
            }

            Fields::Unit => {
                let fields = UnitFields::default();
                let info = UnparsedPatternInfo {
                    pattern: pattern.as_str(),
                    class,
                    target: info.target,
                    fields,
                };

                info.parse()
                    .and_then(ValidatedPatternInfo::emit)
                    .map_err(|err| Error::new(span, err.to_string()))
            }
        }
    } else {
        let new_info = generate_pattern(info, variant_span)
            .map_err(|err| Error::new(variant_span, err.to_string()))?;
        compile_pattern_parser(new_info, decl_span, variant_span)
    }
}

struct EnumInfo<'src> {
    data: &'src syn::DataEnum,
    ident: &'src Ident,
    attrs: &'src Vec<Attribute>,
    generics: &'src Generics,
}

fn expand_enum(input: EnumInfo) -> Result<TokenStream> {
    let enum_ident = &input.ident;
    let enum_attrs = EnumAttrs::from_attrs(input.attrs)?;
    if enum_attrs.syntax_class.is_none() {
        return crate::legacy_codegen::expand_enum(
            input.data,
            input.ident,
            input.attrs,
            input.generics,
        );
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
    let mut variant_data: Vec<(Ident, TokenStream2)> = vec![];
    for variant in &data.variants {
        let vattrs = VariantAttrs::from_variant(variant)?;
        let field_attrs: Vec<FieldAttrs> = variant
            .fields
            .iter()
            .map(FieldAttrs::from_field)
            .collect::<Result<_>>()?;

        // Validate: #[parse_with] on a #[delegate] variant
        if vattrs.delegate {
            for fa in &field_attrs {
                if fa.parse_with.is_some() {
                    return Err(Error::new(
                        vattrs.delegate_span.unwrap(),
                        "#[delegate] cannot be combined with #[parse_with] on a field",
                    ));
                }
            }
        }

        let pattern_compilation_info = PatternCompilationInfo {
            fields: &variant.fields,
            pattern_info: vattrs.pattern,
            target: PatternTarget::Variant {
                ident: variant.ident.clone(),
            },
            class: enum_attrs.syntax_class,
        };

        let ident_and_parser =
            compile_pattern_parser(pattern_compilation_info, enum_ident.span(), variant.span())?;

        variant_data.push(ident_and_parser);
    }

    let parser_generics = generics_with_lifetime(input.generics, &src_lifetime);
    let (impl_generics, _, where_clause) = parser_generics.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();

    // Build the alternative parser. A left-nested `.or()` chain can blow the
    // trait-resolver recursion limit (observed at ~41 variants), while
    // chumsky's tuple `choice` is flat but only implemented up to 26 elements.
    // Above that, group variants into inner tuple choices and wrap those in an
    // outer tuple choice, keeping the type shallow without boxing.
    let var_names: Vec<&Ident> = variant_data.iter().map(|(n, _)| n).collect();
    let variant_bindings: Vec<&TokenStream2> = variant_data.iter().map(|(_, b)| b).collect();
    let or_chain = if variant_data.len() == 1 {
        // chumsky's `choice` is only impl'd for tuples of size 2..=26;
        // a single-variant enum just yields its sole binding directly.
        let only = &variant_data[0].0;
        quote! { #only }
    } else if variant_data.len() <= 26 {
        quote! { ::chumsky::primitive::choice((#(#var_names),*)) }
    } else {
        let chunks: Vec<TokenStream2> = var_names
            .chunks(26)
            .map(|chunk| {
                let parts = chunk.iter();
                quote! { ::chumsky::primitive::choice((#(#parts),*)) }
            })
            .collect();
        quote! { ::chumsky::primitive::choice((#(#chunks),*)) }
    };

    let output = quote! {
        impl #impl_generics ::vihaco_parser_core::Parse<#src_lifetime> for #enum_ident #ty_generics #where_clause {
            fn parser() -> impl ::chumsky::Parser<
                #src_lifetime,
                &#src_lifetime str,
                Self,
                ::chumsky::extra::Err<::chumsky::error::Simple<#src_lifetime, char>>,
            > {
                use ::chumsky::Parser as _;
                #(#variant_bindings)*
                #or_chain
            }
        }
    };

    Ok(output.into())
}

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Enum(e) => expand_enum(EnumInfo {
            data: e,
            ident: &input.ident,
            attrs: &input.attrs,
            generics: &input.generics,
        }),
        //Data::Struct(s) => expand_struct(StructInfo { s }),
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
