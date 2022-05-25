use std::env;
use cmake;

fn main() {
    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap();
    let openssl_dir =
        env::var("OPENSSL_ROOT_DIR").ok().map(|x| format!("{}/lib", x)).or(
            pkg_config::probe_library("openssl").ok()
                .map(|lib| lib.link_paths.first().unwrap().display().to_string())
        ).unwrap();

    let clang;
    let clangpp;
    if let Ok(llvm_path) = env::var("LLVM_PATH") {
        clang = format!("{}/bin/clang", llvm_path);
        clangpp = format!("{}/bin/clang++", llvm_path);

        println!("cargo:rustc-link-search=native={}/lib/x86_64-unknown-linux-gnu", llvm_path);
    } else {
        clang = "clang".to_string();
        clangpp = "clang++".to_string();
    }

    if target.contains("darwin") {
        let dst = cmake::Config::new("ton")
            .uses_cxx11()
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", clang)
            .define("CMAKE_CXX_COMPILER", clangpp)
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("tonlibjson")
            .build();

        println!("cargo:rustc-link-search=native={}/build/tonlib", dst.display());
        println!("cargo:rustc-link-lib=dylib=tonlibjson");

        return;
    }

    println!("cargo:rustc-link-arg=-fuse-ld=lld");

    println!("cargo:rustc-link-search=native={}", openssl_dir);
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=static=ssl");

    let dst;
    if profile == "debug" {
        dst = cmake::Config::new("ton")
            .uses_cxx11()
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", clang)
            .define("CMAKE_CXX_COMPILER", clangpp)
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("tonlibjson_static")
            .build();
    } else {
        dst = cmake::Config::new("ton")
            .uses_cxx11()
            .cxxflag("-flto")
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", clang)
            .define("CMAKE_CXX_COMPILER", clangpp)
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .cxxflag("-fuse-ld=lld")
            .cxxflag("-Wno-error=unused-command-line-argument")
            .build_target("tonlibjson_static")
            .build();
    }

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
    println!("cargo:rustc-link-lib=static=tonlibjson");
    println!("cargo:rustc-link-lib=static=tonlibjson_private");
    println!("cargo:rustc-link-lib=static=tonlibjson_static");
}
