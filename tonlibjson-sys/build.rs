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
    eprintln!("ton dir is {ton_dir}");
    println!("cargo:rerun-if-changed={ton_dir}/CMakeLists.txt");
    println!("cargo:rerun-if-changed=build.rs");

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target_os == "linux" {
        println!("cargo:rustc-link-lib=static=c++");
    }

    let openssl_paths = if !cfg!(feature = "bundled") {
        let (lib_dir, include_dir) = if let Ok(root) = env::var("OPENSSL_ROOT_DIR") {
            (
                PathBuf::from(format!("{root}/lib")),
                PathBuf::from(format!("{root}/include")),
            )
        } else {
            let openssl = pkg_config::probe_library("openssl").unwrap();
            (
                openssl.link_paths.first().unwrap().to_path_buf(),
                openssl.include_paths.first().unwrap().to_path_buf(),
            )
        };
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=crypto");
        println!("cargo:rustc-link-lib=static=ssl");
        Some((lib_dir, include_dir))
    } else {
        None
    };

    // sodium is always built from third-party/sodium by CMake (BuildSodium.cmake has no skip mechanism)
    // linking is done after cmake build in tonlibjson/tonemulator sections

    let secp256k1_paths = if !cfg!(feature = "bundled") {
        let secp256k1 = pkg_config::probe_library("libsecp256k1").unwrap();
        let secp256k1_dir = secp256k1.link_paths.first().unwrap().to_path_buf();
        println!("cargo:rustc-link-search=native={}", secp256k1_dir.display());
        println!("cargo:rustc-link-lib=static=secp256k1");
        Some((secp256k1_dir, secp256k1.include_paths.first().unwrap().to_path_buf()))
    } else {
        None
    };

    // On macOS, always use bundled zlib (pkg_config returns incomplete paths)
    let use_bundled_zlib = cfg!(feature = "bundled") || target_os == "macos";
    let zlib_paths = if !use_bundled_zlib {
        let zlib = pkg_config::probe_library("zlib").unwrap();
        let zlib_dir = zlib.link_paths.first().unwrap().to_path_buf();
        println!("cargo:rustc-link-search=native={}", zlib_dir.display());
        println!("cargo:rustc-link-lib=static=z");
        let zlib_include_dir = zlib.include_paths.first().map(|p| p.to_path_buf());
        Some((zlib_dir, zlib_include_dir))
    } else {
        None
    };

    let mut cfg = Config::new(out_dir.join(ton_dir));
    cfg.define("TON_ONLY_TONLIB", "ON")
        .define("CMAKE_C_COMPILER", "clang")
        // without CMAKE_BUILD_TYPE=Release got error
        // clang++: error: no such file or directory: '&&'
        // clang++: error: no such file or directory: 'dsymutil'
        // clang++: error: no such file or directory: 'generate_common'
        .define("CMAKE_BUILD_TYPE", "Release")
        // QUIC will require openssl 3.5+
        // but its not easy to install
        // https://github.com/getgems-io/ton-grpc/actions/runs/21245238777/job/61132760447?pr=1483
        .define("USE_QUIC", "OFF")
        .define("CMAKE_CXX_COMPILER", "clang++")
        .define("PORTABLE", "ON")
        .define("TONLIBJSON_STATIC", "ON")
        .define("EMULATOR_STATIC", "ON")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("TON_ARCH", target_arch)
        .cxxflag("-std=c++14")
        .cxxflag("-stdlib=libc++")
        .always_configure(true)
        .very_verbose(false);

    // Pass system OpenSSL to CMake so BuildOpenSSL.cmake skips bundled build
    if let Some((ref lib_dir, ref include_dir)) = openssl_paths {
        cfg.define(
            "OPENSSL_CRYPTO_LIBRARY",
            lib_dir.join("libcrypto.a").to_str().unwrap(),
        );
        cfg.define(
            "OPENSSL_SSL_LIBRARY",
            lib_dir.join("libssl.a").to_str().unwrap(),
        );
        cfg.define("OPENSSL_INCLUDE_DIR", include_dir.to_str().unwrap());
    }

    // Pass system secp256k1 to CMake so BuildSECP256K1.cmake skips bundled build
    if let Some((ref secp256k1_dir, ref secp256k1_include_dir)) = secp256k1_paths {
        let secp256k1_lib = secp256k1_dir.join("libsecp256k1.a");
        cfg.define("SECP256K1_LIBRARY", secp256k1_lib.to_str().unwrap());
        cfg.define("SECP256K1_INCLUDE_DIR", secp256k1_include_dir.to_str().unwrap());
    }

    // Pass system zlib to CMake so BuildZlib.cmake skips bundled build
    if let Some((ref zlib_dir, ref zlib_include_dir)) = zlib_paths {
        let zlib_lib = zlib_dir.join("libz.a");
        cfg.define("ZLIB_FOUND", "1");
        cfg.define("ZLIB_LIBRARY", zlib_lib.to_str().unwrap());
        cfg.define("ZLIB_LIBRARIES", zlib_lib.to_str().unwrap());
        if let Some(ref include_dir) = zlib_include_dir {
            cfg.define("ZLIB_INCLUDE_DIR", include_dir.to_str().unwrap());
        }
    }

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

    let use_old_private_lib = !cfg!(feature = "testnet");

    if cfg!(feature = "tonlibjson") {
        let dst = cfg.build_target("tonlibjson").build();

        if cfg!(feature = "bundled") {
            println!(
                "cargo:rustc-link-search=native={}/build/third-party/openssl/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=crypto");
            println!("cargo:rustc-link-lib=static=ssl");

            println!(
                "cargo:rustc-link-search=native={}/build/third-party/secp256k1/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=secp256k1");
        }

        // zlib is bundled when feature "bundled" is active or on macOS
        if use_bundled_zlib {
            println!(
                "cargo:rustc-link-search=native={}/build/third-party/zlib/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=z");
        }

        // sodium is always bundled (BuildSodium.cmake has no skip mechanism)
        println!(
            "cargo:rustc-link-search=native={}/build/third-party/sodium/lib",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=sodium");

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
            println!("cargo:rustc-link-lib=static={item}")
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
        if use_old_private_lib {
            // tonlibjson_private was removed from this commit
            // https://github.com/ton-blockchain/ton/commit/ddb173b16f4ff8fb314175b9751720dbfc79e77e
            // but still used on master branch
            println!("cargo:rustc-link-lib=static=tonlibjson_private");
        }
        println!("cargo:rustc-link-lib=static=tonlibjson");
    }

    if cfg!(feature = "tonemulator") {
        let dst = cfg.build_target("emulator").build();

        if cfg!(feature = "bundled") {
            println!(
                "cargo:rustc-link-search=native={}/build/third-party/openssl/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=crypto");
            println!("cargo:rustc-link-lib=static=ssl");

            println!(
                "cargo:rustc-link-search=native={}/build/third-party/secp256k1/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=secp256k1");
        }

        // zlib is bundled when feature "bundled" is active or on macOS
        if use_bundled_zlib {
            println!(
                "cargo:rustc-link-search=native={}/build/third-party/zlib/lib",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=z");
        }

        // sodium is always bundled (BuildSodium.cmake has no skip mechanism)
        println!(
            "cargo:rustc-link-search=native={}/build/third-party/sodium/lib",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=sodium");

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
