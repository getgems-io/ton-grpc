use std::env;
use cmake::Config;

fn main() {
    let is_release = env::var("PROFILE").unwrap() == "release";
    let target = env::var("TARGET").unwrap();
    let is_darwin = target == "x86_64-apple-darwin";

    let openssl_dir = env::var("OPENSSL_ROOT_DIR")
        .ok()
        .map(|x| format!("{}/lib", x))
        .or_else(
            || pkg_config::probe_library("openssl")
                .ok()
                .map(|lib| lib.link_paths.first().unwrap().display().to_string())
        ).unwrap();

    let is_testnet = cfg!(feature = "testnet");
    let ton_dir = if cfg!(feature = "testnet") { "ton-testnet" } else { "ton" };
    let build_tonlibjson = cfg!(feature = "tonlibjson");
    let build_emulator = cfg!(feature = "tonemulator");

    eprintln!("ton dir is {}", ton_dir);

    if is_darwin {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=static=c++");
    }

    println!("cargo:rustc-link-search=native={}", openssl_dir);
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=static=ssl");

    if is_testnet {
        println!("cargo:rustc-link-lib=static=sodium");
        println!("cargo:rustc-link-lib=static=secp256k1");
    }

    let target_arch = "x86-64";

    if build_tonlibjson {
        let dst= if !is_darwin && is_release {
            Config::new(ton_dir)
                .define("TON_ONLY_TONLIB", "ON")
                .define("CMAKE_C_COMPILER", "clang")
                .define("CMAKE_CXX_COMPILER", "clang++")
                .define("CMAKE_CXX_STANDARD", "14")
                .define("BUILD_SHARED_LIBS", "OFF")
                .define("SODIUM_USE_STATIC_LIBS", "OFF")
                .define("PORTABLE", "ON")
                .define("TON_ARCH", target_arch)
                .cxxflag("-std=c++14")
                .cxxflag("-stdlib=libc++")
                .cxxflag("-flto")
                .cxxflag("-fuse-ld=lld")
                .define("CMAKE_EXE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_MODULE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_SHARED_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .uses_cxx11()
                .build_target("tonlibjson")
                .build()
        } else {
            Config::new(ton_dir)
                .uses_cxx11()
                .define("TON_ONLY_TONLIB", "ON")
                .define("CMAKE_C_COMPILER", "clang")
                .define("CMAKE_CXX_COMPILER", "clang++")
                .define("CMAKE_CXX_STANDARD", "14")
                .define("BUILD_SHARED_LIBS", "OFF")
                .define("SODIUM_USE_STATIC_LIBS", "OFF")
                .define("PORTABLE", "ON")
                .define("TON_ARCH", target_arch)
                .cxxflag("-std=c++14")
                .cxxflag("-stdlib=libc++")
                .build_target("tonlibjson")
                .build()
        };

        if is_testnet {
            println!("cargo:rustc-link-search=native={}/build/third-party/blst", dst.display());
            println!("cargo:rustc-link-lib=static=blst");
        }

        for item in ["tdnet", "keys", "tdactor", "tl-utils", "tdutils"] {
            println!("cargo:rustc-link-search=native={}/build/{}", dst.display(), item);
            println!("cargo:rustc-link-lib=static={}", item)
        }

        println!("cargo:rustc-link-search=native={}/build/adnl", dst.display());
        println!("cargo:rustc-link-lib=static=adnllite");

        println!("cargo:rustc-link-search=native={}/build/lite-client", dst.display());
        println!("cargo:rustc-link-lib=static=lite-client-common");

        println!("cargo:rustc-link-search=native={}/build/crypto", dst.display());
        println!("cargo:rustc-link-lib=static=ton_crypto");
        if is_testnet {
            println!("cargo:rustc-link-lib=static=ton_crypto_core");
        }
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
        println!("cargo:rustc-link-lib=static=tddb_utils");
        println!("cargo:rustc-link-lib=static=tdutils");
        println!("cargo:rustc-link-lib=static=tl-lite-utils");

        println!("cargo:rustc-link-search=native={}/build/emulator", dst.display());
        println!("cargo:rustc-link-lib=static=emulator_static");

        println!("cargo:rustc-link-search=native={}/build/tonlib", dst.display());
        println!("cargo:rustc-link-lib=static=tonlib");
        println!("cargo:rustc-link-lib=static=tonlibjson_private");
        println!("cargo:rustc-link-lib=static=tonlibjson");
    }

    if build_emulator {
        let dst = if !is_darwin && is_release {
            Config::new(ton_dir)
                .define("TON_ONLY_TONLIB", "ON")
                .define("CMAKE_C_COMPILER", "clang")
                .define("CMAKE_CXX_COMPILER", "clang++")
                .define("CMAKE_CXX_STANDARD", "14")
                .define("BUILD_SHARED_LIBS", "OFF")
                .define("SODIUM_USE_STATIC_LIBS", "OFF")
                .define("PORTABLE", "ON")
                .define("TON_ARCH", target_arch)
                .cxxflag("-std=c++14")
                .cxxflag("-stdlib=libc++")
                .cxxflag("-flto")
                .cxxflag("-fuse-ld=lld")
                .define("CMAKE_EXE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_MODULE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .define("CMAKE_SHARED_LINKER_FLAGS_INIT", "-fuse-ld=lld")
                .uses_cxx11()
                .build_target("emulator")
                .build()
        } else {
            Config::new(ton_dir)
                .uses_cxx11()
                .define("TON_ONLY_TONLIB", "ON")
                .define("CMAKE_C_COMPILER", "clang")
                .define("CMAKE_CXX_COMPILER", "clang++")
                .define("CMAKE_CXX_STANDARD", "14")
                .define("BUILD_SHARED_LIBS", "OFF")
                .define("SODIUM_USE_STATIC_LIBS", "OFF")
                .define("PORTABLE", "ON")
                .define("TON_ARCH", target_arch)
                .cxxflag("-std=c++14")
                .cxxflag("-stdlib=libc++")
                .build_target("emulator")
                .build()
        };

        if is_testnet {
            println!("cargo:rustc-link-search=native={}/build/third-party/blst", dst.display());
            println!("cargo:rustc-link-lib=static=blst");
        }

        println!("cargo:rustc-link-search=native={}/build/crypto", dst.display());
        println!("cargo:rustc-link-lib=static=ton_crypto");
        println!("cargo:rustc-link-lib=static=ton_block");
        println!("cargo:rustc-link-lib=static=smc-envelope");

        println!("cargo:rustc-link-search=native={}/build/emulator", dst.display());
        println!("cargo:rustc-link-lib=static=emulator_static");
        println!("cargo:rustc-link-lib=static=emulator");
    }
}
