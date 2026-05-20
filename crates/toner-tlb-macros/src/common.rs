use darling::{FromDeriveInput, FromField, FromVariant, util::SpannedValue};
use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::{
    DeriveInput, Expr, Field, Fields, GenericParam, Generics, Ident, Result, Type, Variant,
    parse_quote, punctuated::Punctuated, token::Comma,
};

pub trait Backend {
    fn ident() -> Ident;
    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream;
    fn validate_field_layer(layer: FieldLayer, span: Span) -> Result<()>;
    fn validate_separate_cell_marker(marker: SeparateCellMarker, span: Span) -> Result<()>;
    fn default_layer() -> FieldLayer;
}

#[derive(Default)]
pub struct ContainerAttrs {
    pub tag: Option<TagValue>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(tlb), supports(struct_any, enum_any))]
struct RawContainer {
    tag: Option<SpannedValue<String>>,
}

#[derive(FromVariant)]
#[darling(attributes(tlb))]
struct RawVariant {
    tag: Option<SpannedValue<String>>,
}

#[derive(FromField)]
#[darling(attributes(tlb))]
pub struct RawField {
    pub ident: Option<Ident>,
    pub cell: darling::util::Flag,
    pub bits: darling::util::Flag,
    #[darling(rename = "as")]
    pub as_ty: Option<Type>,
    pub args: Option<Expr>,
    pub separate_cell_start: darling::util::Flag,
    pub separate_cell_end: darling::util::Flag,
}

pub fn parse_container_attrs(input: &DeriveInput) -> Result<ContainerAttrs> {
    let raw = RawContainer::from_derive_input(input)?;
    let tag = raw
        .tag
        .map(|s| TagValue::parse_str(&s, s.span()))
        .transpose()?;
    Ok(ContainerAttrs { tag })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TagFormat {
    Binary,
    Hex,
}

pub struct TagValue {
    bits: Vec<bool>,
    format: TagFormat,
    span: Span,
}

impl TagValue {
    pub fn parse_str(s: &str, span: Span) -> Result<Self> {
        let (rest, format) = match s.as_bytes() {
            [b'0', b'b', ..] => (&s[2..], TagFormat::Binary),
            [b'$', ..] => (&s[1..], TagFormat::Binary),
            [b'0', b'x', ..] => (&s[2..], TagFormat::Hex),
            [b'#', ..] => (&s[1..], TagFormat::Hex),
            _ => return Err(syn::Error::new(span, "tag must start with 0b, 0x, # or $")),
        };
        let bits = match format {
            TagFormat::Binary => rest
                .chars()
                .map(|c| match c {
                    '0' => Ok(false),
                    '1' => Ok(true),
                    other => Err(syn::Error::new(
                        span,
                        format!("invalid binary digit: {other}"),
                    )),
                })
                .collect::<Result<Vec<bool>>>()?,
            TagFormat::Hex => {
                let val = u64::from_str_radix(rest, 16)
                    .map_err(|e| syn::Error::new(span, format!("invalid hex: {e}")))?;
                (0..rest.len() * 4)
                    .rev()
                    .map(|i| (val >> i) & 1 == 1)
                    .collect()
            }
        };
        Ok(Self { bits, format, span })
    }

    pub fn bit_len(&self) -> usize {
        self.bits.len()
    }
    pub fn format(&self) -> TagFormat {
        self.format
    }
    pub fn bits(&self) -> &[bool] {
        &self.bits
    }
    pub fn span(&self) -> Span {
        self.span
    }

    fn as_u64(&self) -> u64 {
        self.bits.iter().fold(0u64, |a, &b| (a << 1) | b as u64)
    }

    pub fn literal(&self) -> Literal {
        format_bits_literal(self.as_u64(), self.bit_len(), self.format)
    }
}

pub fn format_bits_literal(val: u64, bit_len: usize, format: TagFormat) -> Literal {
    let s = match format {
        TagFormat::Binary => format!("0b{val:0>bit_len$b}", bit_len = bit_len),
        TagFormat::Hex => format!("0x{val:0>hex_digits$x}", hex_digits = bit_len.div_ceil(4)),
    };
    s.parse()
        .expect("formatted bits literal must be a valid Rust literal")
}

pub fn tag_int_type(bit_len: usize, span: Span) -> Result<TokenStream> {
    let ty = match bit_len {
        0..=8 => quote! { u8 },
        9..=16 => quote! { u16 },
        17..=32 => quote! { u32 },
        _ => {
            return Err(syn::Error::new(
                span,
                format!("tags longer than 32 bits are not supported (got {bit_len})"),
            ));
        }
    };
    Ok(ty)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FieldLayer {
    Cell,
    Bits,
}

pub struct FieldMode {
    pub layer: FieldLayer,
    pub as_ty: Option<Type>,
    pub args: Option<Expr>,
}

impl FieldMode {
    pub fn from_raw<B: Backend>(raw: RawField, span: Span) -> Result<Self> {
        let layer = match (raw.cell.is_present(), raw.bits.is_present()) {
            (true, true) => {
                return Err(syn::Error::new(
                    span,
                    "field cannot be both `cell` and `bits`; choose one",
                ));
            }
            (true, false) => Some(FieldLayer::Cell),
            (false, true) => Some(FieldLayer::Bits),
            (false, false) => None,
        };
        if layer.is_none() && raw.as_ty.is_some() {
            return Err(syn::Error::new(
                span,
                "`as = \"...\"` requires a layer marker; add `cell` or `bits`",
            ));
        }
        let layer = layer.unwrap_or_else(B::default_layer);
        B::validate_field_layer(layer, span)?;
        Ok(Self {
            layer,
            as_ty: raw.as_ty,
            args: raw.args,
        })
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SeparateCellMarker {
    #[default]
    None,
    Start,
    End,
    Both,
}

pub struct VariantInfo {
    pub ident: Ident,
    pub tag: TagValue,
    pub fields: Punctuated<Field, Comma>,
}

pub fn parse_variant(variant: &Variant) -> Result<VariantInfo> {
    let raw = RawVariant::from_variant(variant)?;
    let tag_str = raw.tag.ok_or_else(|| {
        syn::Error::new_spanned(
            &variant.ident,
            "enum variant must have #[tlb(tag = \"...\")]",
        )
    })?;
    let tag = TagValue::parse_str(&tag_str, tag_str.span())?;
    let fields = match &variant.fields {
        Fields::Named(f) => f.named.clone(),
        Fields::Unit => Punctuated::new(),
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "tuple variants are not supported",
            ));
        }
    };
    Ok(VariantInfo {
        ident: variant.ident.clone(),
        tag,
        fields,
    })
}

pub fn extend_generics_with_de(generics: &Generics) -> (TokenStream, TokenStream, TokenStream) {
    let mut extended = generics.clone();
    extended
        .params
        .insert(0, GenericParam::Lifetime(parse_quote!('de)));
    let (impl_g, _, _) = extended.split_for_impl();
    let (_, ty_g, where_g) = generics.split_for_impl();
    (quote! { #impl_g }, quote! { #ty_g }, quote! { #where_g })
}

pub fn split_generics(generics: &Generics) -> (TokenStream, TokenStream, TokenStream) {
    let (impl_g, ty_g, where_g) = generics.split_for_impl();
    (quote! { #impl_g }, quote! { #ty_g }, quote! { #where_g })
}
