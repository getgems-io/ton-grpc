use cmake;

fn main() {
    eprintln!("Build tonlibjson");

    let dst = cmake::Config::new("ton")
        .build_target("tonlibjson")
        .build();

    eprintln!("{}", dst.display());
    println!("cargo:rustc-link-search=native={}/build/tonlib", dst.display());
    println!("cargo:rustc-link-lib=dylib=tonlibjson");
}
