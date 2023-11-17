use std::collections::{BTreeMap, BTreeSet};
use std::{env, fs};
use std::fmt::format;
use std::path::{Path, PathBuf};
use anyhow::bail;
use quote::{format_ident, quote};
use syn::{Expr, Ident, Meta, MetaList};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheme_path = if cfg!(testnet) {
        Path::new("../tonlibjson-sys/ton-testnet/tl/generate/scheme/tonlib_api.tl")
    } else {
        Path::new("../tonlibjson-sys/ton/tl/generate/scheme/tonlib_api.tl")
    };

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", scheme_path.to_string_lossy());

    Generator::from(scheme_path, "generated.rs")
        .add_type("ton.blockIdExt", vec!["Serialize", "Deserialize", "Eq", "PartialEq", "Hash", "new"])
        .add_type("sync", vec!["Serialize"])
        .add_type("blocks.getBlockHeader", vec!["Serialize", "Hash", "PartialEq", "Eq", "new"])
        .generate()?;

    Ok(())
}

struct Generator {
    input: PathBuf,
    output: PathBuf,
    types: Vec<(String, Vec<String>)>
}

impl Generator {
    fn from<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Self {
        let input: PathBuf = input.as_ref().to_path_buf();
        let output: PathBuf = output.as_ref().to_path_buf();

        Self { input, output, types: vec![] }
    }

    fn add_type(mut self, name: &str, derives: Vec<&str>) -> Self {
        self.types.push((name.to_owned(), derives.into_iter().map(|s| s.to_owned()).collect()));

        self
    }

    fn generate(self) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(self.input)?;

        let combinators = tl_parser::parse(&content)?;
        let btree: BTreeMap<_, _> = combinators
            .into_iter()
            .map(|combinator| (combinator.id().to_owned(), combinator))
            .collect();

        let mut generated = BTreeSet::new();
        let mut formatted = String::new();

        for (type_ident, derives) in self.types.into_iter() {
            let definition = btree.get(&type_ident).unwrap();

            eprintln!("definition = {:?}", definition);

            let id = definition.id();
            let struct_name = structure_ident(definition.id());

            let mut traits = vec!["Debug".to_owned(), "Default".to_owned(), "Clone".to_owned()];
            traits.extend(derives);

            let derives = format!("derive({})", traits.join(","));

            let t = syn::parse_str::<MetaList>(&derives)?;
            // eprintln!("t = {:?}", t);
            //
            // bail!("HUI");

            // let derives: Vec<syn::Ident> = derives.into_iter().map(|d| format_ident!("{}", d)).collect();

            let fields: Vec<_> = definition.fields().iter().map(|field| {
                eprintln!("field = {:?}", field);
                let field_name = format_ident!("{}", field.id().clone().unwrap());
                let field_type = format_ident!("{}", structure_ident(field.field_type().clone().unwrap()));

                // TODO[akostylev0]: just write custom wrappers for primitive types
                if field_type == "Int32" || field_type == "Int64" {
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

        let out_dir = env::var_os("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir)
            .join(self.output);

        eprintln!("dest_path = {:?}", dest_path);

        fs::write(dest_path, formatted).unwrap();

        Ok(())
    }
}


fn uppercase_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}


fn structure_ident(s: &str) -> Ident {
    format_ident!("{}",s.split('.').map(uppercase_first_letter).collect::<Vec<_>>().join(""))
}
