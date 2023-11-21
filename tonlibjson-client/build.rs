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
        .add_type("accountAddress", vec!["Deserialize", "Serialize"])
        .add_type("ton.blockId", vec!["Serialize", "Deserialize", "Eq", "PartialEq", "Hash", "new"])
        .add_type("ton.blockIdExt", vec!["Serialize", "Deserialize", "Eq", "PartialEq", "Hash", "new"])
        .add_type("blocks.header", vec!["Deserialize"])
        .add_type("blocks.shortTxId", vec!["Deserialize"])
        .add_type("blocks.masterchainInfo", vec!["Deserialize", "Eq", "PartialEq"])
        .add_type("internal.transactionId", vec!["Serialize", "Deserialize", "Eq", "PartialEq"])
        .add_type("raw.message", vec!["Deserialize"])
        .add_type("raw.transaction", vec!["Deserialize"])
        .add_type("raw.extMessageInfo", vec!["Deserialize"])
        .add_type("blocks.accountTransactionId", vec!["Serialize", "Deserialize"])
        .add_type("blocks.shards", vec!["Deserialize"])
        .add_type("blocks.transactions", vec!["Deserialize"])

        .add_type("msg.dataEncrypted", vec!["Serialize", "Deserialize"])

        .add_type("msg.dataRaw", vec!["Serialize", "Deserialize"])
        .add_type("msg.dataText", vec!["Serialize", "Deserialize"])
        .add_type("msg.dataDecryptedText", vec!["Serialize", "Deserialize"])
        .add_type("msg.dataEncryptedText", vec!["Serialize", "Deserialize"])
        .add_type("msg.decryptWithProof", vec!["Serialize", "Deserialize"])

        .add_type("tvm.slice", vec!["Serialize", "Deserialize"])
        .add_type("tvm.cell", vec!["Serialize", "Deserialize"])
        .add_type("tvm.numberDecimal", vec!["Serialize", "Deserialize"])
        .add_type("tvm.tuple", vec!["Serialize", "Deserialize"])
        .add_type("tvm.list", vec!["Serialize", "Deserialize"])

        .add_type("tvm.stackEntrySlice", vec!["Serialize", "Deserialize"])
        .add_type("tvm.stackEntryCell", vec!["Serialize", "Deserialize"])
        .add_type("tvm.stackEntryNumber", vec!["Serialize", "Deserialize"])
        .add_type("tvm.stackEntryTuple", vec!["Serialize", "Deserialize"])
        .add_type("tvm.stackEntryList", vec!["Serialize", "Deserialize"])
        .add_type("tvm.stackEntryUnsupported", vec!["Serialize", "Deserialize"])

        .add_type("smc.info", vec!["Deserialize"])

        .add_type("sync", vec!["Default", "Serialize"])
        .add_type("blocks.getBlockHeader", vec!["Serialize", "Hash", "PartialEq", "Eq", "new"])

        .add_boxed_type("msg.Data")
        .add_boxed_type("tvm.StackEntry")
        .add_boxed_type("tvm.Number")
        .add_boxed_type("tvm.Tuple")
        .add_boxed_type("tvm.List")

        .generate()?;

    Ok(())
}

struct Generator {
    input: PathBuf,
    output: PathBuf,
    types: Vec<(String, Vec<String>)>,
    boxed_types: Vec<String>,
}

impl Generator {
    fn from<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Self {
        let input: PathBuf = input.as_ref().to_path_buf();
        let output: PathBuf = output.as_ref().to_path_buf();

        Self { input, output, types: vec![], boxed_types: vec![] }
    }

    fn add_type(mut self, name: &str, derives: Vec<&str>) -> Self {
        self.types.push((name.to_owned(), derives.into_iter().map(|s| s.to_owned()).collect()));

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
                .or_insert(Vec::new())
                .push(combinator.to_owned());
        }

        let btree: HashMap<_, _> = combinators
            .into_iter()
            .map(|combinator| (combinator.id().to_owned(), combinator))
            .collect();

        let mut generated = HashSet::new();
        let mut formatted = String::new();

        for (type_ident, derives) in self.types.into_iter() {
            let definition = btree.get(&type_ident).unwrap();

            eprintln!("definition = {:?}", definition);

            let id = definition.id();
            let struct_name = structure_ident(definition.id());

            let mut traits = vec!["Debug".to_owned(), "Clone".to_owned()];
            traits.extend(derives);

            let derives = format!("derive({})", traits.join(","));
            let t = syn::parse_str::<MetaList>(&derives)?;

            // eprintln!("t = {:?}", t);

            // let derives: Vec<syn::Ident> = derives.into_iter().map(|d| format_ident!("{}", d)).collect();

            let fields: Vec<_> = definition.fields().iter().map(|field| {
                eprintln!("field = {:?}", field);
                let field_name = format_ident!("{}", field.id().clone().unwrap());
                let mut deserialize_number_from_string = false; // TODO[akostylev0]
                let field_type: Box<dyn ToTokens> = if field.field_type().is_some_and(|typ| typ == "#") {
                    deserialize_number_from_string = true;
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

                    eprintln!("gen = {:?}", gen);
                    let r = syn::parse_str::<GenericArgument>(&gen).unwrap();
                    eprintln!("r = {:?}", r);

                    Box::new(r)
                } else {
                    let field_type = field.field_type();
                    if field_type.is_some_and(|s| s == "int32" || s == "int64" || s == "int53" || s == "int256")  {
                        deserialize_number_from_string = true;
                    }

                    // TODO[akostylev0]
                    Box::new(format_ident!("{}", structure_ident(field_type.clone().unwrap())))
                };


                // // TODO[akostylev0]: just write custom wrappers for primitive types
                if deserialize_number_from_string {
                    quote! {
                        #[serde(deserialize_with = "deserialize_number_from_string")]
                        pub #field_name: #field_type
                    }
                } else {
                    quote! {
                    pub #field_name: #field_type
                    }
                }
            }).collect();

            let output = quote! {
                #[#t]
                #[serde(tag = "@type", rename = #id)]
                pub struct #struct_name {
                    #(#fields),*
                }
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

            let output = quote! {
                #[derive(Deserialize, Serialize, Clone, Debug)]
                #[serde(untagged)]
                pub enum #struct_name {
                    #(#fields),*
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
    let (ns, name) = s.rsplit_once(".").unwrap_or(("", s));

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
