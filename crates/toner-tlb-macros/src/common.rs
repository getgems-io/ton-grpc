use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{
    Fields, Ident, LitStr, Result, Type, punctuated::Punctuated, spanned::Spanned, token::Comma,
};

#[derive(Default)]
pub struct ContainerAttrs {
    pub tag: Option<TagValue>,
    pub ensure_empty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TagFormat {
    Binary,
    Hex,
}

pub struct TagValue {
    pub bits: Vec<bool>,
    pub format: TagFormat,
}

impl TagValue {
    pub fn parse_from_str(s: &str) -> Result<Self> {
        let (bits, format) = if let Some(rest) = s.strip_prefix("0b") {
            (parse_binary_digits(rest)?, TagFormat::Binary)
        } else if let Some(rest) = s.strip_prefix("0x") {
            (parse_hex_to_bits(rest)?, TagFormat::Hex)
        } else if let Some(rest) = s.strip_prefix('#') {
            (parse_hex_to_bits(rest)?, TagFormat::Hex)
        } else if let Some(rest) = s.strip_prefix('$') {
            (parse_binary_digits(rest)?, TagFormat::Binary)
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "tag must start with 0b, 0x, # or $",
            ));
        };

        Ok(TagValue { bits, format })
    }

    pub fn bit_len(&self) -> usize {
        self.bits.len()
    }

    pub fn as_u64(&self) -> u64 {
        self.bits
            .iter()
            .fold(0u64, |acc, &b| (acc << 1) | (b as u64))
    }
}

pub fn make_literal(val: u64, bit_len: usize, format: TagFormat) -> proc_macro2::Literal {
    match format {
        TagFormat::Binary => {
            let s = format!("0b{:0>width$b}", val, width = bit_len);
            s.parse::<proc_macro2::Literal>().unwrap()
        }
        TagFormat::Hex => {
            let hex_digits = bit_len.div_ceil(4);
            let s = format!("0x{:0>width$x}", val, width = hex_digits);
            s.parse::<proc_macro2::Literal>().unwrap()
        }
    }
}

fn parse_binary_digits(s: &str) -> Result<Vec<bool>> {
    s.chars()
        .map(|c| match c {
            '0' => Ok(false),
            '1' => Ok(true),
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid binary digit: {c}"),
            )),
        })
        .collect()
}

fn parse_hex_to_bits(s: &str) -> Result<Vec<bool>> {
    let val = u64::from_str_radix(s, 16).map_err(|e| {
        syn::Error::new(proc_macro2::Span::call_site(), format!("invalid hex: {e}"))
    })?;
    let bit_count = s.len() * 4;
    Ok((0..bit_count).rev().map(|i| (val >> i) & 1 == 1).collect())
}

pub fn parse_container_attrs(attrs: &[syn::Attribute]) -> Result<ContainerAttrs> {
    let mut result = ContainerAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                result.tag = Some(TagValue::parse_from_str(&lit.value())?);
            } else if meta.path.is_ident("ensure_empty") {
                result.ensure_empty = true;
            } else {
                return Err(meta.error("unknown tlb attribute"));
            }
            Ok(())
        })?;
    }
    Ok(result)
}

pub struct FieldMode {
    pub kind: FieldModeKind,
    pub args: Option<syn::Expr>,
}

pub enum FieldModeKind {
    Parse,
    ParseAs(Type),
    Unpack,
    UnpackAs(Type),
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SeparateCellMarker {
    #[default]
    None,
    Start,
    End,
    Both,
}

#[derive(Default)]
pub struct FieldAttrs {
    pub mode: Option<FieldMode>,
    pub separate_cell: SeparateCellMarker,
}

pub fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<FieldAttrs> {
    let mut kind: Option<FieldModeKind> = None;
    let mut args: Option<syn::Expr> = None;
    let mut start = false;
    let mut end = false;
    for attr in attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("parse") {
                if meta.input.peek(syn::Token![=]) {
                    return Err(meta.error("use `parse_as` for typed parse"));
                }
                kind = Some(FieldModeKind::Parse);
            } else if meta.path.is_ident("parse_as") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let ty: Type = syn::parse_str(&lit.value())?;
                kind = Some(FieldModeKind::ParseAs(ty));
            } else if meta.path.is_ident("unpack") {
                if meta.input.peek(syn::Token![=]) {
                    return Err(meta.error("use `unpack_as` for typed unpack"));
                }
                kind = Some(FieldModeKind::Unpack);
            } else if meta.path.is_ident("unpack_as") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let ty: Type = syn::parse_str(&lit.value())?;
                kind = Some(FieldModeKind::UnpackAs(ty));
            } else if meta.path.is_ident("args") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let expr: syn::Expr = syn::parse_str(&lit.value())?;
                args = Some(expr);
            } else if meta.path.is_ident("separate_cell_start") {
                start = true;
            } else if meta.path.is_ident("separate_cell_end") {
                end = true;
            } else {
                return Err(meta.error("unknown tlb field attribute"));
            }
            Ok(())
        })?;
    }

    let separate_cell = match (start, end) {
        (false, false) => SeparateCellMarker::None,
        (true, false) => SeparateCellMarker::Start,
        (false, true) => SeparateCellMarker::End,
        (true, true) => SeparateCellMarker::Both,
    };

    Ok(FieldAttrs {
        mode: kind.map(|kind| FieldMode { kind, args }),
        separate_cell,
    })
}

pub struct FieldEntry {
    pub binding: Ident,
    pub mode: FieldMode,
    pub context: String,
    pub separate_cell: SeparateCellMarker,
    pub span: proc_macro2::Span,
}

/// Models a flat field run or a `^[ ... ]` (TLB "separate cell") block,
/// loaded as a fresh child cell and required to be fully consumed.
pub enum FieldSection {
    Flat(Vec<FieldEntry>),
    SeparateCell(Vec<FieldEntry>),
}

pub fn group_fields_into_sections(entries: Vec<FieldEntry>) -> Result<Vec<FieldSection>> {
    let mut sections: Vec<FieldSection> = Vec::new();
    let mut current_flat: Vec<FieldEntry> = Vec::new();
    let mut current_separate: Option<Vec<FieldEntry>> = None;

    for entry in entries {
        match (current_separate.as_mut(), entry.separate_cell) {
            (None, SeparateCellMarker::None) => current_flat.push(entry),
            (None, SeparateCellMarker::Start) => {
                if !current_flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut current_flat)));
                }
                current_separate = Some(vec![entry]);
            }
            (None, SeparateCellMarker::Both) => {
                if !current_flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut current_flat)));
                }
                sections.push(FieldSection::SeparateCell(vec![entry]));
            }
            (None, SeparateCellMarker::End) => {
                return Err(syn::Error::new(
                    entry.span,
                    "`separate_cell_end` without a preceding `separate_cell_start`",
                ));
            }
            (Some(_), SeparateCellMarker::Start) => {
                return Err(syn::Error::new(
                    entry.span,
                    "nested `separate_cell_start` is not allowed; close the previous block with `separate_cell_end` first",
                ));
            }
            (Some(_), SeparateCellMarker::Both) => {
                return Err(syn::Error::new(
                    entry.span,
                    "cannot have both `separate_cell_start` and `separate_cell_end` on a field that is already inside an open separate-cell block",
                ));
            }
            (Some(buf), SeparateCellMarker::None) => buf.push(entry),
            (Some(_), SeparateCellMarker::End) => {
                let mut buf = current_separate.take().unwrap();
                buf.push(entry);
                sections.push(FieldSection::SeparateCell(buf));
            }
        }
    }

    if let Some(open) = current_separate {
        let span = open
            .first()
            .map(|f| f.span)
            .unwrap_or_else(proc_macro2::Span::call_site);
        return Err(syn::Error::new(
            span,
            "`separate_cell_start` without a matching `separate_cell_end`",
        ));
    }

    if !current_flat.is_empty() {
        sections.push(FieldSection::Flat(current_flat));
    }

    Ok(sections)
}

#[derive(Clone, Copy)]
pub enum DefaultFieldMode {
    Parse,
    Unpack,
}

impl DefaultFieldMode {
    fn into_kind(self) -> FieldModeKind {
        match self {
            DefaultFieldMode::Parse => FieldModeKind::Parse,
            DefaultFieldMode::Unpack => FieldModeKind::Unpack,
        }
    }
}

pub fn gen_field_entries(
    fields: impl IntoIterator<Item = (Ident, String, proc_macro2::Span, Vec<syn::Attribute>)>,
    default: DefaultFieldMode,
) -> Result<Vec<FieldEntry>> {
    let mut entries = Vec::new();
    for (binding, context, span, attrs) in fields {
        let parsed = parse_field_attrs(&attrs)?;
        let mode = parsed.mode.unwrap_or(FieldMode {
            kind: default.into_kind(),
            args: None,
        });
        entries.push(FieldEntry {
            binding,
            mode,
            context,
            separate_cell: parsed.separate_cell,
            span,
        });
    }
    Ok(entries)
}

pub fn collect_named_field_inputs(
    fields: &Punctuated<syn::Field, Comma>,
) -> Vec<(Ident, String, proc_macro2::Span, Vec<syn::Attribute>)> {
    fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().unwrap().clone();
            let context = ident.to_string();
            let span = f.span();
            (ident, context, span, f.attrs.clone())
        })
        .collect()
}

pub fn collect_tuple_field_inputs(
    fields: &Punctuated<syn::Field, Comma>,
) -> Vec<(Ident, String, proc_macro2::Span, Vec<syn::Attribute>)> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let ident = format_ident!("__field_{}", i);
            let context = i.to_string();
            let span = f.span();
            (ident, context, span, f.attrs.clone())
        })
        .collect()
}

pub fn int_type_for_bits(bit_len: usize) -> Result<TokenStream> {
    use quote::quote;
    match bit_len {
        0..=8 => Ok(quote! { u8 }),
        9..=16 => Ok(quote! { u16 }),
        17..=32 => Ok(quote! { u32 }),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("tags longer than 32 bits are not supported (got {bit_len})"),
        )),
    }
}

pub struct VariantInfo {
    pub ident: Ident,
    pub tag: TagValue,
    pub fields: Punctuated<syn::Field, Comma>,
}

pub fn parse_variant_info(variant: &syn::Variant) -> Result<VariantInfo> {
    let mut tag = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                tag = Some(TagValue::parse_from_str(&lit.value())?);
            } else {
                return Err(meta.error("unknown tlb variant attribute"));
            }
            Ok(())
        })?;
    }

    let tag = tag.ok_or_else(|| {
        syn::Error::new_spanned(
            &variant.ident,
            "enum variant must have #[tlb(tag = \"...\")]",
        )
    })?;

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

pub struct TagTreeNode {
    pub leaf: Option<usize>,
    pub children: BTreeMap<bool, TagTreeNode>,
}

impl TagTreeNode {
    pub fn new() -> Self {
        Self {
            leaf: None,
            children: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, bits: &[bool], variant_idx: usize) {
        if bits.is_empty() {
            self.leaf = Some(variant_idx);
            return;
        }
        self.children
            .entry(bits[0])
            .or_insert_with(TagTreeNode::new)
            .insert(&bits[1..], variant_idx);
    }

    pub fn min_leaf_depth(&self) -> usize {
        if self.leaf.is_some() {
            return 0;
        }
        1 + self
            .children
            .values()
            .map(|c| c.min_leaf_depth())
            .min()
            .unwrap_or(0)
    }
}
