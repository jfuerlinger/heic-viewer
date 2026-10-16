//! Links libheif dynamically (LGPL: the library stays a separately replaceable DLL/dylib).
//!
//! Environment variables:
//! - `HEIF_LIB_DIR`: directory containing the import library (`heif.lib`) or `libheif.dylib/.so`;
//!   on Windows/CI this is `vcpkg_installed/<triplet>/lib`.
//! - `HEIF_LIB_NAME`: library name to link (default `heif`).

use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=HEIF_LIB_DIR");
    println!("cargo:rerun-if-env-changed=HEIF_LIB_NAME");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let lib_name = env::var("HEIF_LIB_NAME").unwrap_or_else(|_| "heif".to_string());

    if let Ok(dir) = env::var("HEIF_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
        // Convenience for local macOS/Linux test runs against a non-system libheif.
        if target_os == "macos" || target_os == "linux" {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
        }
    } else if target_os == "macos" {
        for dir in ["/opt/homebrew/lib", "/usr/local/lib"] {
            if Path::new(dir).join("libheif.dylib").exists() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }

    println!("cargo:rustc-link-lib=dylib={lib_name}");
}
