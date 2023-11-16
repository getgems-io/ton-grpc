use anyhow::Result;
use tl_parser::parse;

fn main() -> Result<()> {
    let schema = include_str!("../../tonlibjson-sys/ton/tl/generate/scheme/tonlib_api.tl");

    let constructors = parse(&schema)?;
    for c in constructors.iter() {
        println!("{:?}", c)
    }

    Ok(())
}
