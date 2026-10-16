//! heif.dll is delay-loaded: Explorer loads us into dllhost.exe, whose DLL search path does not
//! include our install directory. The provider preloads heif.dll by full path before first use
//! (see src/loader.rs), so a missing/broken libheif becomes an HRESULT instead of a load failure.

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=HEIF_DLL_NAME");
    let dll = env::var("HEIF_DLL_NAME").unwrap_or_else(|_| "heif.dll".to_string());
    println!("cargo:rustc-env=HEIF_DLL_NAME={dll}");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=/DELAYLOAD:{dll}");
        println!("cargo:rustc-link-lib=delayimp");
    }
}
