use std::collections::{HashMap, HashSet};
use std::{env, fs};
use std::path::{Path, PathBuf};
use anyhow::bail;
use quote::{format_ident, quote, ToTokens};
use syn::{GenericArgument, Ident, MetaList};
use tl_parser::Combinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheme_path = if cfg!(testnet) {
        Path::new("../tonlibjson-sys/ton-testnet/tl/generate/scheme/tonlib_api.tl")
    } else {
        Path::new("../tonlibjson-sys/ton/tl/generate/scheme/tonlib_api.tl")
    };

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", scheme_path.to_string_lossy());

    Generator::from(scheme_path, "generated.rs")
        .add_type("ok", vec!["Deserialize"])
        .add_type_full("accountAddress", configure_type()
            .derives(vec!["Clone", "Deserialize", "Serialize"])
            .field("account_address", configure_field()
                .optional()
                .serialize_with("serialize_none_as_empty")
                .deserialize_with("deserialize_empty_as_none")
                .build())
            .build()
        )
        .add_type("ton.blockId", vec!["Clone", "Serialize", "Deserialize", "Eq", "PartialEq", "Hash", "new"])
        .add_type("ton.blockIdExt", vec!["Clone", "Serialize", "Deserialize", "Eq", "PartialEq", "Hash", "new"])
        .add_type("blocks.header", vec!["Clone", "Deserialize"])
        .add_type("blocks.shortTxId", vec!["Clone", "Deserialize"])
        .add_type("blocks.masterchainInfo", vec!["Clone", "Deserialize", "Eq", "PartialEq"])
        .add_type("internal.transactionId", vec!["Clone", "Serialize", "Deserialize", "Eq", "PartialEq"])
        .add_type_full("raw.transactions", configure_type()
            .derives(vec!["Deserialize"])
            .field("previous_transaction_id", configure_field()
                .optional()
                .deserialize_with("deserialize_default_as_none")
                .build())
            .build())
        .add_type("raw.message", vec!["Deserialize"])
        .add_type("raw.transaction", vec!["Deserialize"])
        .add_type("raw.extMessageInfo", vec!["Deserialize"])
        .add_type_full("raw.fullAccountState", configure_type()
            .derives(vec!["Deserialize"])
            .field("balance", configure_field()
                .optional()
                .deserialize_with("deserialize_ton_account_balance")
                .build())
            .field("last_transaction_id", configure_field()
                .optional()
                .deserialize_with("deserialize_default_as_none")
                .build()
            )
            .build()
        )
        .add_type("blocks.accountTransactionId", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("blocks.shards", vec!["Clone", "Deserialize"])
        .add_type("blocks.transactions", vec!["Deserialize"])

        .add_type("msg.dataEncrypted", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("msg.dataRaw", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("msg.dataText", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("msg.dataDecryptedText", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("msg.dataEncryptedText", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("msg.decryptWithProof", vec!["Clone", "Serialize", "Deserialize"])

        .add_type("tvm.slice", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.cell", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.numberDecimal", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.tuple", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.list", vec!["Clone", "Serialize", "Deserialize"])

        .add_type("tvm.stackEntrySlice", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.stackEntryCell", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.stackEntryNumber", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.stackEntryTuple", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.stackEntryList", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("tvm.stackEntryUnsupported", vec!["Clone", "Serialize", "Deserialize"])

        .add_type("smc.info", vec!["Deserialize"])
        .add_type("smc.methodIdNumber", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("smc.methodIdName", vec!["Clone", "Serialize", "Deserialize"])

        .add_type("sync", vec!["Default", "Serialize"])
        .add_type("blocks.getBlockHeader", vec!["Clone", "Serialize", "Hash", "PartialEq", "Eq", "new"])
        .add_type("getShardAccountCell", vec!["Clone", "Serialize", "new"])
        .add_type("getShardAccountCellByTransaction", vec!["Clone", "Serialize", "new"])
        .add_type("raw.getAccountState", vec!["Clone", "Serialize", "new"])
        .add_type("raw.getAccountStateByTransaction", vec!["Clone", "Serialize", "new"])
        .add_type("getAccountState", vec!["Clone", "Serialize", "new"])
        .add_type("blocks.getMasterchainInfo", vec!["Clone", "Default", "Serialize", "new"])
        .add_type("blocks.lookupBlock", vec!["Clone", "Serialize", "new", "Hash", "Eq", "PartialEq"])
        .add_type("blocks.getShards", vec!["Clone", "Serialize", "new"])
        .add_type("blocks.getTransactions", vec!["Clone", "Serialize", "new"])
        .add_type("raw.sendMessage", vec!["Serialize", "new"])
        .add_type("raw.sendMessageReturnHash", vec!["Serialize", "new"])
        .add_type("smc.load", vec!["Clone", "Serialize", "new"])
        .add_type("smc.runGetMethod", vec!["Clone", "Serialize", "new"])
        .add_type("smc.runResult", vec!["Deserialize"])
        .add_type("fullAccountState", vec!["Deserialize"])
        .add_type("uninited.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("pchan.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("rwallet.limit", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("rwallet.config", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("rwallet.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("dns.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("wallet.highload.v1.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("wallet.highload.v2.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("wallet.v3.accountState", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("raw.accountState", vec!["Clone", "Serialize", "Deserialize"])

        .add_type("pchan.stateInit", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("pchan.stateClose", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("pchan.statePayout", vec!["Clone", "Serialize", "Deserialize"])
        .add_type("pchan.config", vec!["Clone", "Serialize", "Deserialize"])

        .add_type_full("raw.getTransactionsV2", configure_type().derives(vec!["Clone", "Serialize", "new"])
            .field("private_key", configure_field().skip().build())
            .build()
        )
        // .add_type("withBlock", vec!["Clone", "Serialize", "new"])

        .add_boxed_type("msg.Data")
        .add_boxed_type("tvm.StackEntry")
        .add_boxed_type("tvm.Number")
        .add_boxed_type("tvm.Tuple")
        .add_boxed_type("tvm.List")
        .add_boxed_type("smc.MethodId")
        .add_boxed_type("ton.BlockIdExt")
        .add_boxed_type("raw.Transactions")
        .add_boxed_type("smc.RunResult")
        .add_boxed_type("smc.Info")
        .add_boxed_type("raw.ExtMessageInfo")
        .add_boxed_type("Ok")
        .add_boxed_type("blocks.Transactions")
        .add_boxed_type("blocks.Shards")
        .add_boxed_type("blocks.Header")
        .add_boxed_type("blocks.MasterchainInfo")
        .add_boxed_type("FullAccountState")
        .add_boxed_type("raw.FullAccountState")
        .add_boxed_type("AccountState")
        .add_boxed_type("pchan.State")

        .add_boxed_type("tvm.Cell")

        .generate()?;

    Ok(())
}

struct Generator {
    input: PathBuf,
    output: PathBuf,
    types: Vec<(String, TypeConfiguration)>,
    boxed_types: Vec<String>,
}


fn configure_type() -> TypeConfigurationBuilder { Default::default() }
fn configure_field() -> FieldConfigurationBuilder { Default::default() }

#[derive(Default)]
struct TypeConfigurationBuilder {
    derives: Vec<String>,
    fields: HashMap<String, FieldConfiguration>
}

struct TypeConfiguration {
    pub derives: Vec<String>,
    pub fields: HashMap<String, FieldConfiguration>
}

#[derive(Default)]
struct FieldConfigurationBuilder {
    skip: bool,
    optional: bool,
    deserialize_with: Option<String>,
    serialize_with: Option<String>
}

#[derive(Default)]
struct FieldConfiguration {
    pub skip: bool,
    pub optional: bool,
    pub deserialize_with: Option<String>,
    pub serialize_with: Option<String>
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
        FieldConfiguration { skip: self.skip, optional: self.optional, deserialize_with: self.deserialize_with, serialize_with: self.serialize_with }
    }
}

impl TypeConfigurationBuilder {
    fn derives(mut self, derives: Vec<&str>) -> Self {
        self.derives = derives.into_iter().map(|s| s.to_owned()).collect();

        self
    }

    fn field(mut self, field: &str, configuration: FieldConfiguration) -> Self {
        self.fields.insert(field.to_owned(), configuration);

        self
    }

    fn build(self) -> TypeConfiguration {
        TypeConfiguration { derives: self.derives, fields: self.fields }
    }
}


impl Generator {
    fn from<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Self {
        let input: PathBuf = input.as_ref().to_path_buf();
        let output: PathBuf = output.as_ref().to_path_buf();

        Self { input, output, types: vec![], boxed_types: vec![] }
    }

    fn add_type(mut self, name: &str, derives: Vec<&str>) -> Self {
        self.types.push((name.to_owned(), configure_type().derives(derives).build()));

        self
    }

    fn add_type_full(mut self, name: &str, configuration: TypeConfiguration) -> Self {
        self.types.push((name.to_owned(), configuration));

        self
    }

    fn add_boxed_type(mut self, name: &str) -> Self {
        self.boxed_types.push(name.to_owned());

        self
    }

    fn generate(self) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(self.input)?;

        let combinators = tl_parser::parse(&content)?;

        let mut map: HashMap<String, Vec<Combinator>> = HashMap::default();
        for combinator in combinators.iter() {
            map.entry(combinator.result_type().to_owned())
                .or_default()
                .push(combinator.to_owned());
        }

        let btree: HashMap<_, _> = combinators
            .into_iter()
            .map(|combinator| (combinator.id().to_owned(), combinator))
            .collect();

        let mut generated = HashSet::new();
        let mut formatted = String::new();

        for (type_ident, configuration) in self.types.into_iter() {
            let definition = btree.get(&type_ident).unwrap();

            eprintln!("definition = {:?}", definition);

            let id = definition.id();
            let struct_name = structure_ident(definition.id());

            let mut traits = vec!["Debug".to_owned()];
            traits.extend(configuration.derives);

            let derives = format!("derive({})", traits.join(","));
            let t = syn::parse_str::<MetaList>(&derives)?;

            // eprintln!("t = {:?}", t);

            // let derives: Vec<syn::Ident> = derives.into_iter().map(|d| format_ident!("{}", d)).collect();

            let fields: Vec<_> = definition.fields()
                .iter()
                .filter(|field| {
                    let default_configuration = FieldConfiguration::default();
                    let field_name = field.id().clone().unwrap();
                    let field_configuration = configuration.fields.get(&field_name).unwrap_or(&default_configuration);

                    !field_configuration.skip
                })
                .map(|field| {
                let default_configuration = FieldConfiguration::default();
                let field_name = field.id().clone().unwrap();
                let field_configuration = configuration.fields.get(&field_name).unwrap_or(&default_configuration);

                eprintln!("field = {:?}", field);
                let field_name = format_ident!("{}", &field_name);
                let mut deserialize_number_from_string = false; // TODO[akostylev0]
                let field_type: Box<dyn ToTokens> = if field.field_type().is_some_and(|typ| typ == "#") {
                    deserialize_number_from_string = true;
                    if field_configuration.optional {
                        Box::new(syn::parse_str::<GenericArgument>("Option<Int31>").unwrap())
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
                        gen = format!("Option<{}>", gen);
                    }
                    Box::new(syn::parse_str::<GenericArgument>(&gen).unwrap())
                } else {
                    let field_type = field.field_type();
                    if field_type.is_some_and(|s| s == "int32" || s == "int64" || s == "int53" || s == "int256")  {
                        deserialize_number_from_string = true;
                    }

                    if field_configuration.optional {
                        let id = format!("Option<{}>", structure_ident(field_type.unwrap()));
                        Box::new(syn::parse_str::<GenericArgument>(&id).unwrap())
                    } else {
                        Box::new(format_ident!("{}", structure_ident(field_type.unwrap())))
                    }
                };

                let serialize_with = if let Some(serialize_with) = &field_configuration.serialize_with { quote! {
                    #[serde(serialize_with = #serialize_with)]
                } } else {
                    quote! {}
                };
                let deserialize_with = if let Some(deserialize_with) = &field_configuration.deserialize_with { quote! {
                    #[serde(deserialize_with = #deserialize_with)]
                } } else {
                    quote! {}
                };

                // // TODO[akostylev0]: just write custom wrappers for primitive types
               if deserialize_number_from_string && deserialize_with.is_empty() {
                   quote! {
                        #serialize_with
                        #[serde(deserialize_with = "deserialize_number_from_string")]
                        pub #field_name: #field_type
                    }
               } else  {
                    quote! {
                        #serialize_with
                        #deserialize_with
                        pub #field_name: #field_type
                   }
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

            eprintln!("tokens = {}", output);

            generated.insert(definition.result_type());
        }

        for type_ident in self.boxed_types.into_iter() {
            let output_name = generate_type_name(&type_ident);
            let struct_name = format_ident!("{}", output_name);

            let types = map.get(&type_ident).unwrap();

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

                quote! {
                    #[derive(Deserialize, Serialize, Clone, Debug)]
                    #[serde(untagged)]
                    pub enum #struct_name {
                        #(#fields),*
                    }
                }
            };

            eprintln!("tokens = {}", output);

            let syntax_tree = syn::parse2(output.clone()).unwrap();
            formatted += &prettyplease::unparse(&syntax_tree);

            eprintln!("tokens = {}", output);
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
        .map(uppercase_first_letter)
        .collect::<Vec<_>>()
        .join("");

    let name = uppercase_first_letter(name);

    format!("{}{}{}", ns_prefix, boxed_prefix, name)
}

fn structure_ident(s: &str) -> Ident {
    format_ident!("{}", generate_type_name(s))
}

fn uppercase_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
