use convert_case::Case::UpperCamel;
use convert_case::{Case, Casing};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs};
use syn::{GenericArgument, Ident, MetaList};
use tl_parser::Combinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheme_path = if cfg!(feature = "testnet") {
        Path::new("../tonlibjson-sys/ton-testnet/tl/generate/scheme/tonlib_api.tl")
    } else {
        Path::new("../tonlibjson-sys/ton/tl/generate/scheme/tonlib_api.tl")
    };

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", scheme_path.to_string_lossy());

    Generator::from(scheme_path, "generated.rs")
        .configure("ok", vec!["Deserialize"])
        .configure("sync", vec!["Default", "Serialize"])
        .configure_full(
            "accountAddress",
            configure_type()
                .derives(vec!["Clone", "Deserialize", "Serialize"])
                .field(
                    "account_address",
                    configure_field()
                        .optional()
                        .serialize_with("serialize_none_as_empty")
                        .deserialize_with("deserialize_empty_as_none")
                        .build(),
                )
                .build(),
        )
        .configure(
            "ton.blockId",
            vec![
                "Clone",
                "Serialize",
                "Deserialize",
                "Eq",
                "PartialEq",
                "Hash",
                "new",
            ],
        )
        .configure(
            "ton.blockIdExt",
            vec![
                "Clone",
                "Serialize",
                "Deserialize",
                "Eq",
                "PartialEq",
                "Hash",
                "new",
            ],
        )
        .configure(
            "blocks.masterchainInfo",
            vec!["Clone", "Deserialize", "Eq", "PartialEq"],
        )
        .configure(
            "internal.transactionId",
            vec!["Clone", "Serialize", "Deserialize", "Eq", "PartialEq"],
        )
        .configure_full(
            "raw.transactions",
            configure_type()
                .derives(vec!["Deserialize"])
                .field(
                    "previous_transaction_id",
                    configure_field()
                        .optional()
                        .deserialize_with("deserialize_default_as_none")
                        .build(),
                )
                .build(),
        )
        .configure_full(
            "raw.transaction",
            configure_type()
                .derives(vec!["Clone", "Serialize", "Deserialize"])
                .field("in_msg", configure_field().optional().build())
                .build(),
        )
        .configure_full(
            "raw.fullAccountState",
            configure_type()
                .derives(vec!["Deserialize"])
                .field(
                    "balance",
                    configure_field()
                        .optional()
                        .deserialize_with("deserialize_ton_account_balance")
                        .build(),
                )
                .field(
                    "last_transaction_id",
                    configure_field()
                        .optional()
                        .deserialize_with("deserialize_default_as_none")
                        .build(),
                )
                .build(),
        )
        .configure(
            "blocks.getBlockHeader",
            vec!["Clone", "Serialize", "Hash", "PartialEq", "Eq", "new"],
        )
        .configure("getShardAccountCell", vec!["Clone", "Serialize", "new"])
        .configure(
            "getShardAccountCellByTransaction",
            vec!["Clone", "Serialize", "new"],
        )
        .configure("raw.getAccountState", vec!["Clone", "Serialize", "new"])
        .configure(
            "raw.getAccountStateByTransaction",
            vec!["Clone", "Serialize", "new"],
        )
        .configure("getAccountState", vec!["Clone", "Serialize", "new"])
        .configure(
            "blocks.getMasterchainInfo",
            vec!["Clone", "Default", "Serialize", "new"],
        )
        .configure(
            "blocks.lookupBlock",
            vec!["Clone", "Serialize", "new", "Hash", "Eq", "PartialEq"],
        )
        .configure("blocks.getShards", vec!["Clone", "Serialize", "new"])
        .configure("blocks.getTransactions", vec!["Clone", "Serialize", "new"])
        .configure("raw.sendMessage", vec!["Serialize", "new"])
        .configure("raw.sendMessageReturnHash", vec!["Serialize", "new"])
        .configure("smc.load", vec!["Clone", "Serialize", "new"])
        .configure("smc.runGetMethod", vec!["Clone", "Serialize", "new"])
        .configure_full(
            "raw.getTransactionsV2",
            configure_type()
                .derives(vec!["Clone", "Serialize", "new"])
                .field("private_key", configure_field().skip().build())
                .build(),
        )
        // .add_type("withBlock", vec!["Clone", "Serialize", "new"])
        .generate()?;

    Ok(())
}

struct Generator {
    input: PathBuf,
    output: PathBuf,
    types: HashMap<String, TypeConfiguration>,
}

fn configure_type() -> TypeConfigurationBuilder {
    Default::default()
}
fn configure_field() -> FieldConfigurationBuilder {
    Default::default()
}

#[derive(Default)]
struct TypeConfigurationBuilder {
    derives: Vec<String>,
    fields: HashMap<String, FieldConfiguration>,
}

struct TypeConfiguration {
    pub derives: Vec<String>,
    pub fields: HashMap<String, FieldConfiguration>,
}

impl Default for TypeConfiguration {
    fn default() -> Self {
        Self {
            derives: vec![
                "Debug".to_owned(),
                "Clone".to_owned(),
                "Serialize".to_owned(),
                "Deserialize".to_owned(),
            ],
            fields: HashMap::new(),
        }
    }
}

#[derive(Default)]
struct FieldConfigurationBuilder {
    skip: bool,
    optional: bool,
    deserialize_with: Option<String>,
    serialize_with: Option<String>,
}

#[derive(Default)]
struct FieldConfiguration {
    pub skip: bool,
    pub optional: bool,
    pub deserialize_with: Option<String>,
    pub serialize_with: Option<String>,
}

impl FieldConfigurationBuilder {
    fn skip(mut self) -> Self {
        self.skip = true;

        self
    }
    fn optional(mut self) -> Self {
        self.optional = true;

        self
    }

    fn deserialize_with(mut self, deserialize_with: &str) -> Self {
        self.deserialize_with = Some(deserialize_with.to_owned());

        self
    }

    fn serialize_with(mut self, serialize_with: &str) -> Self {
        self.serialize_with = Some(serialize_with.to_owned());

        self
    }

    fn build(self) -> FieldConfiguration {
        FieldConfiguration {
            skip: self.skip,
            optional: self.optional,
            deserialize_with: self.deserialize_with,
            serialize_with: self.serialize_with,
        }
    }
}

impl TypeConfigurationBuilder {
    fn derives(mut self, derives: Vec<&str>) -> Self {
        self.derives = derives.into_iter().map(|s| s.to_owned()).collect();
        self.derives.push("Debug".to_owned());

        self
    }

    fn field(mut self, field: &str, configuration: FieldConfiguration) -> Self {
        self.fields.insert(field.to_owned(), configuration);

        self
    }

    fn build(self) -> TypeConfiguration {
        TypeConfiguration {
            derives: self.derives,
            fields: self.fields,
        }
    }
}

impl Generator {
    fn from<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Self {
        let input: PathBuf = input.as_ref().to_path_buf();
        let output: PathBuf = output.as_ref().to_path_buf();

        Self {
            input,
            output,
            types: Default::default(),
        }
    }

    fn configure(mut self, name: &str, derives: Vec<&str>) -> Self {
        self.types
            .insert(name.to_owned(), configure_type().derives(derives).build());

        self
    }

    fn configure_full(mut self, name: &str, configuration: TypeConfiguration) -> Self {
        self.types.insert(name.to_owned(), configuration);

        self
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

        let skip_list: Vec<String> = vec![
            "Vector t",
            "Bool",
            "Int32",
            "Int53",
            "Int64",
            "Int256",
            "Bytes",
            "SecureString",
            "SecureBytes",
            "Object",
            "Function",
        ]
        .into_iter()
        .map(|s| s.to_owned())
        .collect();

        for (type_ident, types) in map {
            eprintln!("type_ident = {type_ident:}");
            if skip_list.contains(&type_ident) {
                continue;
            }

            let output_name = generate_type_name(&type_ident);
            let struct_name = format_ident!("{}", output_name);

            let output = if types
                .iter()
                .filter(|combinator| !combinator.is_functional())
                .count()
                == 1
            {
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

                quote! {
                    #[derive(Deserialize, Serialize, Clone, Debug)]
                    #[serde(untagged)]
                    pub enum #struct_name {
                        #(#fields),*
                    }
                }
            };

            eprintln!("tokens = {output}");

            let syntax_tree = syn::parse2(output.clone()).unwrap();
            formatted += &prettyplease::unparse(&syntax_tree);

            eprintln!("tokens = {output}");

            for definition in types.into_iter() {
                if definition.is_builtin()
                    || definition.id() == "vector"
                    || definition.id() == "int256"
                {
                    continue;
                }

                let default = TypeConfiguration::default();
                let configuration = self.types.get(definition.id()).unwrap_or(&default);

                eprintln!("definition = {definition:?}");

                let id = definition.id();
                let struct_name = structure_ident(definition.id());

                let derives = format!("derive({})", configuration.derives.join(","));
                let t = syn::parse_str::<MetaList>(&derives)?;

                let fields: Vec<_> = definition
                    .fields()
                    .iter()
                    .filter(|field| {
                        let default_configuration = FieldConfiguration::default();
                        let field_name = field.id().unwrap();
                        let field_configuration = configuration
                            .fields
                            .get(field_name)
                            .unwrap_or(&default_configuration);

                        !field_configuration.skip
                    })
                    .map(|field| {
                        let default_configuration = FieldConfiguration::default();
                        let field_name = field.id().unwrap().to_case(Case::Snake);
                        let field_configuration = configuration
                            .fields
                            .get(&field_name)
                            .unwrap_or(&default_configuration);

                        eprintln!("field = {field:?}");
                        let field_name = format_ident!("{}", &field_name);
                        let mut deserialize_number_from_string = false; // TODO[akostylev0]
                        let field_type: Box<dyn ToTokens> = if field
                            .field_type()
                            .is_some_and(|typ| typ == "#")
                        {
                            deserialize_number_from_string = true;
                            if field_configuration.optional {
                                Box::new(
                                    syn::parse_str::<GenericArgument>("Option<Int31>").unwrap(),
                                )
                            } else {
                                Box::new(format_ident!("{}", "Int31"))
                            }
                        } else if field.type_is_polymorphic() {
                            let type_name = generate_type_name(field.field_type().unwrap());
                            let type_variables = field.type_variables().unwrap();
                            let args: Vec<_> = type_variables
                                .into_iter()
                                .map(|s| generate_type_name(&s))
                                .collect();

                            let mut gen = format!("{}<{}>", type_name, args.join(","));
                            if field.type_is_optional() || field_configuration.optional {
                                gen = format!("Option<{gen}>");
                            }
                            Box::new(syn::parse_str::<GenericArgument>(&gen).unwrap())
                        } else {
                            let field_type = field.field_type();
                            if field_type.is_some_and(|s| {
                                s == "int32" || s == "int64" || s == "int53" || s == "int256"
                            }) {
                                deserialize_number_from_string = true;
                            }

                            if field_configuration.optional {
                                let id =
                                    format!("Option<{}>", structure_ident(field_type.unwrap()));
                                Box::new(syn::parse_str::<GenericArgument>(&id).unwrap())
                            } else {
                                Box::new(format_ident!("{}", structure_ident(field_type.unwrap())))
                            }
                        };

                        let serialize_with =
                            if let Some(serialize_with) = &field_configuration.serialize_with {
                                quote! {
                                    #[serde(serialize_with = #serialize_with)]
                                }
                            } else {
                                quote! {}
                            };
                        let deserialize_with =
                            if let Some(deserialize_with) = &field_configuration.deserialize_with {
                                quote! {
                                    #[serde(deserialize_with = #deserialize_with)]
                                }
                            } else {
                                quote! {}
                            };

                        // // TODO[akostylev0]: just write custom wrappers for primitive types
                        if deserialize_number_from_string && deserialize_with.is_empty() {
                            quote! {
                                #serialize_with
                                #[serde(default)]
                                #[serde(deserialize_with = "deserialize_number_from_string")]
                                pub #field_name: #field_type
                            }
                        } else {
                            quote! {
                                #serialize_with
                                #deserialize_with
                                pub #field_name: #field_type
                            }
                        }
                    })
                    .collect();

                let traits = if definition.is_functional() {
                    let result_name =
                        format_ident!("{}", generate_type_name(definition.result_type()));
                    quote! {
                        impl Functional for #struct_name {
                            type Result = #result_name;
                        }
                    }
                } else {
                    quote! {}
                };

                let output = quote! {
                    #[#t]
                    #[serde(tag = "@type", rename = #id)]
                    pub struct #struct_name {
                        #(#fields),*
                    }

                    #traits
                };

                let syntax_tree = syn::parse2(output.clone()).unwrap();
                formatted += &prettyplease::unparse(&syntax_tree);
            }
        }

        let out_dir = env::var_os("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir).join(self.output);

        eprintln!("dest_path = {dest_path:?}");

        fs::write(dest_path, formatted).unwrap();

        Ok(())
    }
}

fn generate_type_name(s: &str) -> String {
    let (ns, name) = s.rsplit_once('.').unwrap_or(("", s));

    let boxed_prefix = if name.starts_with(|c: char| c.is_uppercase()) {
        "Boxed"
    } else {
        ""
    };

    let ns_prefix = ns
        .split('.')
        .map(|f| f.to_case(UpperCamel))
        .collect::<Vec<_>>()
        .join("");

    format!("{}{}{}", ns_prefix, boxed_prefix, name.to_case(UpperCamel))
}

fn structure_ident(s: &str) -> Ident {
    format_ident!("{}", generate_type_name(s))
}
