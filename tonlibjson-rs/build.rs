use std::env;
use cmake;

fn main() {
    let is_static = env::var("CARGO_FEATURE_STATIC").unwrap_or_default() == "1";
    // let is_lto = env::var("CARGO_PROFILE_RELEASE_LTO").unwrap_or_default() == "1";
    let is_lto = false;

    eprintln!("{}, {}", is_static, is_lto);

    if !is_static {
        let dst = cmake::Config::new("ton")
            .uses_cxx11()
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", "clang")
            .define("CMAKE_CXX_COMPILER", "clang++")
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("tonlibjson")
            .build();

        println!("cargo:rustc-link-search=native={}/build/tonlib", dst.display());
        println!("cargo:rustc-link-lib=dylib=tonlibjson");

        return;
    }

    let openssl_dir = pkg_config::probe_library("openssl").ok()
            .map(|lib| lib.link_paths.first().unwrap().display().to_string()).or_else(
        || env::var("OPENSSL_ROOT_DIR").ok().map(|x| format!("{}/lib", x))
    ).unwrap();

    if is_lto {
        println!("cargo:rustc-link-arg=-fuse-ld=lld");
    }

    println!("cargo:rustc-link-search=native={}", openssl_dir);
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=static=ssl");

    let dst= if is_lto {
        cmake::Config::new("ton")
            .uses_cxx11()
            .cxxflag("-flto")
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", "clang")
            .define("CMAKE_CXX_COMPILER", "clang++")
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .cxxflag("-fuse-ld=lld")
            .cxxflag("-Wno-error=unused-command-line-argument")
            .build_target("tonlibjson_static")
            .build()
    } else {
        cmake::Config::new("ton")
            .uses_cxx11()
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", "clang")
            .define("CMAKE_CXX_COMPILER", "clang++")
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("tonlibjson_static")
            .build()
    };

    println!("cargo:rustc-link-lib=static=c++");

    for item in vec!("tdnet", "keys", "tdactor", "tl-utils", "tdutils") {
        println!("cargo:rustc-link-search=native={}/build/{}", dst.display(), item);
        println!("cargo:rustc-link-lib=static={}", item)
    }

    println!("cargo:rustc-link-search=native={}/build/adnl", dst.display());
    println!("cargo:rustc-link-lib=static=adnllite");

    println!("cargo:rustc-link-search=native={}/build/lite-client", dst.display());
    println!("cargo:rustc-link-lib=static=lite-client-common");

    println!("cargo:rustc-link-search=native={}/build/crypto", dst.display());
    println!("cargo:rustc-link-lib=static=ton_crypto");
    println!("cargo:rustc-link-lib=static=ton_block");
    println!("cargo:rustc-link-lib=static=smc-envelope");

    println!("cargo:rustc-link-search=native={}/build/tl", dst.display());
    println!("cargo:rustc-link-lib=static=tl_api");
    println!("cargo:rustc-link-lib=static=tl_lite_api");
    println!("cargo:rustc-link-lib=static=tl_tonlib_api");
    println!("cargo:rustc-link-lib=static=tl_tonlib_api_json");

    println!("cargo:rustc-link-search=native={}/build/tddb", dst.display());
    println!("cargo:rustc-link-lib=static=tddb_utils");

    println!("cargo:rustc-link-search=native={}/build/third-party/crc32c", dst.display());
    println!("cargo:rustc-link-lib=static=crc32c");

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=tdactor");
    println!("cargo:rustc-link-lib=static=tddb");
    println!("cargo:rustc-link-lib=static=tddb_utils");
    println!("cargo:rustc-link-lib=static=tdutils");

    println!("cargo:rustc-link-lib=static=tl-lite-utils");

    println!("cargo:rustc-link-search=native={}/build/tonlib", dst.display());
    println!("cargo:rustc-link-lib=static=tonlib");
    println!("cargo:rustc-link-lib=static=tonlibjson_private");
    println!("cargo:rustc-link-lib=static=tonlibjson_static");
}
