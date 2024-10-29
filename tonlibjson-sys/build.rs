use std::env;
use std::fs::{copy, create_dir_all};
use std::path::{Path, PathBuf};

use cmake::Config;
use walkdir::WalkDir;

fn main() {
    let use_native_arch =
        env::var("TONLIBJSON_SYS_TARGET_CPU_NATIVE").is_ok_and(|v| v == "1" || v == "true");
    let use_lld = env::var("TONLIBJSON_SYS_LLD").is_ok_and(|v| v == "1" || v == "true");
    let use_lto = env::var("TONLIBJSON_SYS_LTO").is_ok_and(|v| v == "1" || v == "true");

    println!("cargo::rerun-if-env-changed=TONLIBJSON_SYS_TARGET_CPU_NATIVE");
    println!("cargo::rerun-if-env-changed=TONLIBJSON_SYS_LLD");
    println!("cargo::rerun-if-env-changed=TONLIBJSON_SYS_LTO");

    let out_dir: PathBuf = env::var("OUT_DIR").unwrap().into();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = if use_native_arch {
        "native".to_owned()
    } else {
        env::var("CARGO_CFG_TARGET_ARCH").unwrap().replace('_', "-")
    };

    let ton_dir = if cfg!(feature = "testnet") {
        "ton-testnet"
    } else {
        "ton"
    };
    eprintln!("ton dir is {}", ton_dir);
    println!("cargo:rerun-if-changed={ton_dir}/CMakeLists.txt");
    println!("cargo:rerun-if-changed=build.rs");

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target_os == "linux" {
        println!("cargo:rustc-link-lib=static=c++");
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
        .to_path_buf();
    println!("cargo:rustc-link-search=native={}", sodium_dir.display());
    println!("cargo:rustc-link-lib=static=sodium");

    let secp256k1_dir = pkg_config::probe_library("libsecp256k1")
        .unwrap()
        .link_paths
        .first()
        .unwrap()
        .to_path_buf();
    println!("cargo:rustc-link-search=native={}", secp256k1_dir.display());
    println!("cargo:rustc-link-lib=static=secp256k1");

    let mut cfg = Config::new(out_dir.join(ton_dir));
    cfg.define("TON_ONLY_TONLIB", "ON")
        .define("CMAKE_C_COMPILER", "clang")
        .define("CMAKE_CXX_COMPILER", "clang++")
        .define("PORTABLE", "ON")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("TON_ARCH", target_arch)
        .cxxflag("-std=c++14")
        .cxxflag("-stdlib=libc++")
        .always_configure(true)
        .very_verbose(false);

    // lz4
    {
        let liblz4 = pkg_config::probe_library("liblz4").unwrap();
        println!(
            "cargo:rustc-link-search=native={}",
            liblz4.link_paths.first().unwrap().display()
        );
        println!("cargo:rustc-link-lib=static=lz4");

        let lz4libs = if target_os == "macos" {
            liblz4
                .link_paths
                .first()
                .unwrap()
                .join(format!("lib{}.a", liblz4.libs.first().unwrap()))
                .to_str()
                .unwrap()
                .to_owned()
        } else {
            liblz4.libs.first().unwrap().to_owned()
        };

        cfg.define("LZ4_FOUND", "1")
            .define("LZ4_LIBRARIES", lz4libs)
            .define(
                "LZ4_INCLUDE_DIRS",
                liblz4.include_paths.first().unwrap().to_str().unwrap(),
            );
    }

    if use_lto {
        cfg.cxxflag("-flto");
    }

    if use_lld {
        cfg.define("CMAKE_EXE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
            .define("CMAKE_MODULE_LINKER_FLAGS_INIT", "-fuse-ld=lld")
            .define("CMAKE_SHARED_LINKER_FLAGS_INIT", "-fuse-ld=lld");
    }

    copy_dir_recursively(
        env::current_dir().unwrap().join(ton_dir),
        out_dir.join(ton_dir),
    )
    .unwrap();

    if cfg!(feature = "tonlibjson") {
        let dst = cfg.build_target("tonlibjson").build();

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
        println!("cargo:rustc-link-lib=static=emulator_static");

        println!(
            "cargo:rustc-link-search=native={}/build/tonlib",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=tonlib");
        println!("cargo:rustc-link-lib=static=tonlibjson_private");
        println!("cargo:rustc-link-lib=static=tonlibjson");
    }

    if cfg!(feature = "tonemulator") {
        let dst = cfg.build_target("emulator").build();

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

fn copy_dir_recursively(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        create_dir_all(dst)?;
    }

    for entry in WalkDir::new(src) {
        let entry = entry.unwrap();

        let target_path = dst.join(entry.path().strip_prefix(src).unwrap());
        if entry.path().is_dir() {
            create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                if !parent.exists() {
                    create_dir_all(parent)?;
                }
            }
            copy(entry.path(), &target_path)?;
        }
    }

    Ok(())
}
