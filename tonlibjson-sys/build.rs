use cmake::Config;
use std::env;

fn main() {
    let is_release = env::var("PROFILE").unwrap() == "release";
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let is_macos = target_os == "macos";
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    let ton_dir = if cfg!(feature = "testnet") {
        "ton-testnet"
    } else {
        "ton"
    };
    eprintln!("ton dir is {}", ton_dir);
    println!("cargo:rerun-if-changed={ton_dir}/CMakeLists.txt");
    println!("cargo::rerun-if-changed=build.rs");

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target_os == "linux" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    let openssl_dir = env::var("OPENSSL_ROOT_DIR").unwrap_or_else(|_| {
        pkg_config::probe_library("openssl")
            .unwrap()
            .link_paths
            .first()
            .unwrap()
            .display()
            .to_string()
    });
    println!("cargo:rustc-link-search=native={}/lib", openssl_dir);
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=static=ssl");

    let sodium_dir = pkg_config::probe_library("libsodium")
        .unwrap()
        .link_paths
        .first()
        .unwrap()
        .to_owned();
    println!("cargo:rustc-link-search=native={}", sodium_dir.display());
    println!("cargo:rustc-link-lib=static=sodium");

    let secp256k1_dir = pkg_config::probe_library("libsecp256k1")
        .unwrap()
        .link_paths
        .first()
        .unwrap()
        .to_owned();
    println!("cargo:rustc-link-search=native={}", secp256k1_dir.display());
    println!("cargo:rustc-link-lib=static=secp256k1");

    if cfg!(feature = "tonlibjson") {
        let mut cfg = Config::new(ton_dir);
        cfg.uses_cxx11()
            .configure_arg("-DTON_ONLY_TONLIB=ON")
            .configure_arg("-DBUILD_SHARED_LIBS=FALSE")
            .configure_arg("-DCMAKE_EXE_LINKER_FLAGS=-L/opt/homebrew/lib")
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", "clang")
            .define("CMAKE_CXX_COMPILER", "clang++")
            .define("CMAKE_CXX_STANDARD", "14")
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("PORTABLE", "ON")
            .define("TON_ARCH", &target_arch)
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("tonlibjson")
            .always_configure(true);
        if is_release {
            cfg.define("CMAKE_BUILD_TYPE", "Release");
            if !is_macos {
                cfg.cxxflag("-flto")
                    .define("CMAKE_EXE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                    .define("CMAKE_MODULE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                    .define("CMAKE_SHARED_LINKER_FLAGS_INIT", "-fuse-ld=lld");
            }
        }
        let dst = cfg.build();

        println!(
            "cargo:rustc-link-search=native={}/build/third-party/blst",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=blst");

        for item in ["tdnet", "keys", "tdactor", "tl-utils", "tdutils"] {
            println!(
                "cargo:rustc-link-search=native={}/build/{}",
                dst.display(),
                item
            );
            println!("cargo:rustc-link-lib=static={}", item)
        }
        println!("cargo:rustc-link-lib=static=tl-lite-utils");

        println!(
            "cargo:rustc-link-search=native={}/build/adnl",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=adnllite");

        println!(
            "cargo:rustc-link-search=native={}/build/lite-client",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=lite-client-common");

        println!(
            "cargo:rustc-link-search=native={}/build/crypto",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=ton_crypto");
        println!("cargo:rustc-link-lib=static=ton_crypto_core");

        println!("cargo:rustc-link-lib=static=ton_block");
        println!("cargo:rustc-link-lib=static=smc-envelope");

        println!("cargo:rustc-link-search=native={}/build/tl", dst.display());
        println!("cargo:rustc-link-lib=static=tl_api");
        println!("cargo:rustc-link-lib=static=tl_lite_api");
        println!("cargo:rustc-link-lib=static=tl_tonlib_api");
        println!("cargo:rustc-link-lib=static=tl_tonlib_api_json");

        println!(
            "cargo:rustc-link-search=native={}/build/tddb",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=tddb_utils");

        println!(
            "cargo:rustc-link-search=native={}/build/third-party/crc32c",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=crc32c");

        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static=tdactor");
        println!("cargo:rustc-link-lib=static=tddb_utils");
        println!("cargo:rustc-link-lib=static=tdutils");
        println!("cargo:rustc-link-lib=static=tl-lite-utils");

        println!(
            "cargo:rustc-link-search=native={}/build/emulator",
            dst.display()
        );
        println!("cargo:rerun-if-changed={}/build/emulator", dst.display());
        println!("cargo:rustc-link-lib=static=emulator_static");

        println!(
            "cargo:rustc-link-search=native={}/build/tonlib",
            dst.display()
        );
        println!("cargo:rerun-if-changed={}/build/tonlib", dst.display());
        println!("cargo:rustc-link-lib=static=tonlib");
        println!("cargo:rustc-link-lib=static=tonlibjson_private");
        println!("cargo:rustc-link-lib=static=tonlibjson");
    }

    if cfg!(feature = "tonemulator") {
        let mut cfg = Config::new(ton_dir);
        cfg.uses_cxx11()
            .define("TON_ONLY_TONLIB", "ON")
            .define("CMAKE_C_COMPILER", "clang")
            .define("CMAKE_CXX_COMPILER", "clang++")
            .define("CMAKE_CXX_STANDARD", "14")
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("PORTABLE", "ON")
            .define("TON_ARCH", target_arch)
            .cxxflag("-std=c++14")
            .cxxflag("-stdlib=libc++")
            .build_target("emulator");
        if !is_macos && is_release {
            cfg.cxxflag("-flto")
                .define("CMAKE_EXE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_MODULE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_SHARED_LINKER_FLAGS_INIT", "-fuse-ld=lld");
        }
        let dst = cfg.build();

        println!(
            "cargo:rustc-link-search=native={}/build/third-party/blst",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=blst");

        println!(
            "cargo:rustc-link-search=native={}/build/crypto",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=ton_crypto");
        println!("cargo:rustc-link-lib=static=ton_block");
        println!("cargo:rustc-link-lib=static=smc-envelope");

        println!(
            "cargo:rustc-link-search=native={}/build/emulator",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=emulator_static");
        println!("cargo:rustc-link-lib=static=emulator");
    }
}
