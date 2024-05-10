use std::collections::HashMap;
use std::{env, fs};
use std::path::{Path, PathBuf};
use syn::{GenericArgument, Ident, MetaList};
use quote::{format_ident, quote, ToTokens};
use convert_case::{Case, Casing};
use convert_case::Case::UpperCamel;
use tl_parser::{Combinator, Condition};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheme_path = if cfg!(testnet) {
        Path::new("../tonlibjson-sys/ton-testnet/tl/generate/scheme/lite_api.tl")
    } else {
        Path::new("../tonlibjson-sys/ton/tl/generate/scheme/lite_api.tl")
    };

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", scheme_path.to_string_lossy());

    Generator::from(scheme_path, "generated.rs")
        .generate()?;

    Ok(())
}

struct Generator {
    input: PathBuf,
    output: PathBuf,
    types: HashMap<String, TypeConfiguration>,
}

struct TypeConfiguration {
    pub derives: Vec<String>
}

impl Default for TypeConfiguration {
    fn default() -> Self {
        Self { derives: vec!["Debug".to_owned(), "Clone".to_owned(), "PartialEq".to_owned(), "Eq".to_owned()] }
    }
}

impl Generator {
    fn from<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Self {
        let input: PathBuf = input.as_ref().to_path_buf();
        let output: PathBuf = output.as_ref().to_path_buf();

        Self { input, output, types: Default::default() }
    }

    fn generate(self) -> anyhow::Result<()> {
        let content = fs::read_to_string(self.input)?;

        let combinators = tl_parser::parse(&content)?;

        let mut map: HashMap<String, Vec<Combinator>> = HashMap::default();
        for combinator in combinators.iter() {
            map.entry(combinator.result_type().to_owned())
                .or_default()
                .push(combinator.to_owned());
        }

        let mut formatted = String::new();

        let skip_list: Vec<String> = vec!["Vector t", "Int32", "Int53", "Int64", "Int128", "Int256", "Bytes", "SecureString", "SecureBytes", "Function"]
            .into_iter().map(|s| s.to_owned()).collect();

        // Boxed Types
        for (type_ident, types) in map {
            eprintln!("type_ident = {:}", type_ident);
            if skip_list.contains(&type_ident) {
                continue;
            }

            let output_name = generate_type_name(&type_ident);
            let struct_name = format_ident!("{}", output_name);

            let output = if types.iter().filter(|combinator| !combinator.is_functional()).count() == 1 {
                let bare_type = types.first().unwrap().id();
                let name = format_ident!("{}", generate_type_name(bare_type));

                quote! {
                    pub type #struct_name = #name;
                }
            } else {
                let fields: Vec<_> = types
                    .iter()
                    .filter(|combinator| !combinator.is_functional())
                    .map(|combinator| {
                        let rename = combinator.id();
                        let field_name = format_ident!("{}", generate_type_name(rename));

                        quote! {
                        #field_name(#field_name)
                    }
                    })
                    .collect();

                let constructor_number_fields: Vec<_> = types
                    .iter()
                    .filter(|combinator| !combinator.is_functional())
                    .map(|combinator| {
                        let rename = combinator.id();
                        let field_name = format_ident!("{}", generate_type_name(rename));

                        quote! {
                            Self::#field_name { .. } => #field_name::CONSTRUCTOR_NUMBER_BE
                        }
                    })
                    .collect();

                let serialize_match: Vec<_> = types
                    .iter()
                    .filter(|combinator| !combinator.is_functional())
                    .map(|combinator| {
                        let rename = combinator.id();
                        let field_name = format_ident!("{}", generate_type_name(rename));

                        quote! {
                            Self::#field_name(inner) => inner.serialize(se)
                        }
                    })
                    .collect();

                let deserialize_match: Vec<_> = types
                    .iter()
                    .filter(|combinator| !combinator.is_functional())
                    .map(|combinator| {
                        let rename = combinator.id();
                        let field_name = format_ident!("{}", generate_type_name(rename));

                        quote! {
                             #field_name::CONSTRUCTOR_NUMBER_BE => { Ok(Self::#field_name(#field_name::deserialize(de)?)) }
                        }
                    })
                    .collect();

                quote! {
                    #[derive(Clone, Debug, PartialEq, Eq)]
                    pub enum #struct_name {
                        #(#fields),*
                    }

                    impl Serialize for #struct_name {
                        fn serialize(&self, se: &mut Serializer) {
                            se.write_constructor_number(match self {
                                #(#constructor_number_fields),*
                            });
                            match self {
                                #(#serialize_match),*
                            }
                        }
                    }

                    impl SerializeBoxed for #struct_name {
                        fn serialize_boxed(&self, se: &mut Serializer) {
                            self.serialize(se);
                        }
                    }

                    impl Deserialize for #struct_name {
                        fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerBoxedError> {
                            let constructor_number = de.parse_constructor_numer()?;

                            Self::deserialize_boxed(constructor_number, de)
                        }
                    }

                    impl DeserializeBoxed for #struct_name {
                        fn deserialize_boxed(constructor_number: u32, de: &mut Deserializer) -> Result<Self, DeserializerBoxedError> {
                            match constructor_number {
                                #(#deserialize_match),*
                                _ => Err(DeserializerBoxedError::UnexpectedConstructorNumber(constructor_number))
                            }
                        }
                    }
                }
            };

            eprintln!("tokens = {}", output);

            let syntax_tree = syn::parse2(output.clone()).unwrap();
            formatted += &prettyplease::unparse(&syntax_tree);

            eprintln!("tokens = {}", output);

            // Bare Types
            for definition in types.into_iter() {
                if definition.is_builtin() || definition.id() == "vector" || definition.id() == "int256" {
                    continue;
                }

                let default = TypeConfiguration::default();
                let configuration = self.types.get(definition.id()).unwrap_or(&default);

                eprintln!("definition = {:?}", definition);

                let struct_name = structure_ident(definition.id());

                let derives = format!("derive({})", configuration.derives.join(","));
                let t = syn::parse_str::<MetaList>(&derives)?;

                let fields: Vec<_> = definition.fields()
                    .iter()
                    .map(|field| {
                        let field_name = field.id().clone().unwrap().to_case(Case::Snake);

                        eprintln!("field = {:?}", field);
                        let field_name = format_ident!("{}", &field_name);
                        let field_type: Box<dyn ToTokens> = if field.field_type().is_some_and(|typ| typ == "#") {
                            Box::new(format_ident!("{}", "Int31"))
                        } else if field.type_is_polymorphic() {
                            let type_name = generate_type_name(field.field_type().unwrap());
                            let type_variables = field.type_variables().unwrap();
                            let args: Vec<_> = type_variables
                                .into_iter()
                                .map(|s| generate_type_name(&s))
                                .collect();

                            let mut gen = format!("{}<{}>", type_name, args.join(","));
                            if field.type_is_optional() {
                                gen = format!("Option<{}>", gen);
                            }
                            Box::new(syn::parse_str::<GenericArgument>(&gen).unwrap())
                        } else {
                            let field_type = field.field_type();
                            if field.type_is_optional() {
                                let id = format!("Option<{}>", structure_ident(field_type.unwrap()));
                                Box::new(syn::parse_str::<GenericArgument>(&id).unwrap())
                            } else {
                                Box::new(format_ident!("{}", structure_ident(field_type.unwrap())))
                            }
                        };

                        quote! {
                            pub #field_name: #field_type
                        }
                    }).collect();

                let serialize_defs: Vec<_> = definition.fields()
                    .iter()
                    .map(|field| {
                        let field_name = field.id().clone().unwrap().to_case(Case::Snake);

                        eprintln!("field = {:?}", field);
                        let field_name_ident = format_ident!("{}", &field_name);

                        match field.type_condition() {
                            None => match field.field_type() {
                                Some("#") => quote! { let mut #field_name_ident = self.#field_name_ident; },
                                // TODO[akostylev0] bool optimization
                                // Some("Bool") => quote! { let #field_name_ident = self.#field_name_ident; },
                                Some("int") => quote! { let #field_name_ident = self.#field_name_ident; },
                                Some("long") => quote! { let #field_name_ident = self.#field_name_ident; },
                                Some("int256") => quote! { let #field_name_ident = &self.#field_name_ident; },
                                Some("bytes") => quote! { let #field_name_ident = &self.#field_name_ident; },
                                Some("string") => quote! { let #field_name_ident = &self.#field_name_ident; },
                                _ => quote! { let #field_name_ident = &self.#field_name_ident; }
                            },
                            Some(Condition { field_ref, bit_selector: Some(bit_selector) }) =>  {
                                let field_ref = format_ident!("{}", &field_ref);
                                quote! {
                                    let #field_name_ident = self.#field_name_ident.as_ref();
                                    if #field_name_ident.is_some() {
                                        #field_ref |= 1 << #bit_selector;
                                    }
                                }
                            },
                            Some(Condition { field_ref: _, bit_selector: None }) => {
                                unimplemented!()
                            }
                        }
                    }).collect();

                let serialize_fields: Vec<_> = definition.fields()
                    .iter()
                    .map(|field| {
                        let field_name = field.id().clone().unwrap().to_case(Case::Snake);

                        eprintln!("field = {:?}", field);
                        let field_name_ident = format_ident!("{}", &field_name);

                        match field.type_condition() {
                            None => match field.field_type() {
                                Some("#") => quote! { se.write_i31(#field_name_ident); },
                                // TODO[akostylev0] bool optimization
                                // Some("Bool") => quote! { se.write_bool(#field_name_ident.into()); },
                                Some("int") => quote! { se.write_i32(#field_name_ident); },
                                Some("long") => quote! { se.write_i64(#field_name_ident); },
                                Some("int256") => quote! { se.write_i256(#field_name_ident); },
                                Some("bytes") => quote! { se.write_bytes(#field_name_ident); },
                                Some("string") => quote! { se.write_string(#field_name_ident); },
                                _ => quote! { #field_name_ident.serialize(se); }
                            },
                            Some(Condition { field_ref: _, bit_selector: Some(_) }) =>  {
                                let inner = match field.field_type() {
                                    Some("#") => quote! { se.write_i31(*value) },
                                    // TODO[akostylev0] bool optimization
                                    // Some("Bool") => quote! { se.write_bool(value.into()) },
                                    Some("int") => quote! { se.write_i32(*value) },
                                    Some("long") => quote! { se.write_i64(*value) },
                                    Some("int256") => quote! { se.write_i256(value) },
                                    Some("bytes") => quote! { se.write_bytes(value) },
                                    Some("string") => quote! { se.write_string(value) },
                                    _ => quote! { value.serialize(se) }
                                };
                                quote! {
                                    match #field_name_ident {
                                        None => {},
                                        Some(value) => #inner,
                                    };
                                }
                            },
                            Some(Condition { field_ref: _, bit_selector: None }) => {
                                unimplemented!()
                            }
                        }
                    }).collect();

                let deserialize_fields: Vec<_> = definition.fields()
                    .iter()
                    .map(|field| {
                        let field_name = field.id().clone().unwrap().to_case(Case::Snake);

                        eprintln!("field = {:?}", field);
                        let field_name_ident = format_ident!("{}", &field_name);

                        let parse_fn = match field.field_type() {
                            Some("#") => quote! { de.parse_i31()? },
                            // TODO[akostylev0] bool optimization
                            // Some("Bool") => quote! { de.parse_bool()?.into() },
                            Some("int") => quote! { de.parse_i32()? },
                            Some("long") => quote! { de.parse_i64()? },
                            Some("int256") => quote! { de.parse_i256()? },
                            Some("bytes") => quote! { de.parse_bytes()? },
                            Some("string") => quote! { de.parse_string()? },
                            _ => {
                                let field_type = format_ident!("{}", structure_ident(field.field_type().unwrap()));
                                quote! { #field_type::deserialize(de)? }
                            }
                        };

                        match field.type_condition() {
                            None => quote! {
                                let #field_name_ident = #parse_fn;
                            },
                            Some(Condition { field_ref, bit_selector: Some(bit_selector) }) =>  {
                                let field_ref = format_ident!("{}", &field_ref);
                                quote! {
                                    let #field_name_ident = if #field_ref & (1 << #bit_selector) > 0 { Some(#parse_fn) } else { None };
                                }
                            },
                            Some(Condition { field_ref, bit_selector: None }) => {
                                let field_ref = format_ident!("{}", &field_ref);
                                quote! {
                                    let #field_name_ident = if #field_ref { Some(#parse_fn) } else { None };
                                }
                            }
                        }
                    }).collect();

                let deserialize_pass: Vec<_> = definition.fields()
                    .iter()
                    .map(|field| {
                        let field_name = field.id().clone().unwrap().to_case(Case::Snake);

                        eprintln!("field = {:?}", field);
                        let field_name_ident = format_ident!("{}", &field_name);

                        quote! {
                            #field_name_ident,
                        }
                    }).collect();

                let traits = if definition.is_functional() {
                    let result_name = format_ident!("{}", generate_type_name(definition.result_type()));
                    quote! {
                        impl Functional for #struct_name {
                            type Result = #result_name;
                        }
                    }
                } else {
                    quote! {}
                };

                let constructor_number_be = definition.constructor_number_be();
                let output = quote! {
                    #[#t]
                    pub struct #struct_name {
                        #(#fields),*
                    }

                    #traits

                    impl #struct_name {
                        const CONSTRUCTOR_NUMBER_BE: u32 = #constructor_number_be;
                    }

                    impl Serialize for #struct_name {
                        #[allow(unused_variables)]
                        fn serialize(&self, se: &mut Serializer) {
                            #(#serialize_defs)*

                            #(#serialize_fields)*
                        }
                    }

                    impl SerializeBoxed for #struct_name {
                        #[allow(unused_variables)]
                        fn serialize_boxed(&self, se: &mut Serializer) {
                            se.write_constructor_number(#constructor_number_be);
                            self.serialize(se)
                        }
                    }

                    impl Deserialize for #struct_name {
                        #[allow(unused_variables)]
                        fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerBoxedError> {
                            #(#deserialize_fields)*

                            Ok(Self {
                                #(#deserialize_pass)*
                            })
                        }
                    }

                    impl DeserializeBoxed for #struct_name {
                        #[allow(unused_variables)]
                        fn deserialize_boxed(constructor_number: u32, de: &mut Deserializer) -> Result<Self, DeserializerBoxedError> {
                            if constructor_number != #constructor_number_be {
                                Err(DeserializerBoxedError::UnexpectedConstructorNumber(constructor_number))
                            } else {
                                Self::deserialize(de)
                            }
                        }
                    }
                };

                eprintln!("{}", output);

                let syntax_tree = syn::parse2(output.clone()).unwrap();
                formatted += &prettyplease::unparse(&syntax_tree);
            }
        }

        let out_dir = env::var_os("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir)
            .join(self.output);

        eprintln!("dest_path = {:?}", dest_path);

        fs::write(dest_path, formatted).unwrap();

        Ok(())
    }
}

fn generate_type_name(s: &str) -> String {
    let (ns, name) = s.rsplit_once('.').unwrap_or(("", s));

    let boxed_prefix = if name.starts_with(|c: char| c.is_uppercase()) {
        "Boxed"
    } else { "" };

    let ns_prefix = ns.split('.')
        .map(|f| f.to_case(UpperCamel))
        .collect::<Vec<_>>()
        .join("");

    format!("{}{}{}", ns_prefix, boxed_prefix, name.to_case(UpperCamel))
}

fn structure_ident(s: &str) -> Ident {
    format_ident!("{}", generate_type_name(s))
}
