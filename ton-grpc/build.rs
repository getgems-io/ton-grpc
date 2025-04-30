// build trigger 5

use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("ton_descriptor.bin"))
        .compile_protos(&["proto/ton.proto"], &["proto"])?;

    Ok(())
}
