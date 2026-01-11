//! Build script for zephyr-python crate.
//!
//! Configures linking to Python library on macOS and Linux.

fn main() {
    // Configure pyo3 build
    pyo3_build_config::use_pyo3_cfgs();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // On macOS with Homebrew Python, add library search path
    if target_os == "macos" {
        // Add library search path for Homebrew Python
        println!(
            "cargo:rustc-link-search=/opt/homebrew/opt/python@3.13/Frameworks/Python.framework/Versions/3.13/lib"
        );
        println!("cargo:rustc-link-lib=python3.13");
    }

    // On Linux (GitHub Actions), link to Python library
    if target_os == "linux" {
        // Link to python3-dev library
        println!("cargo:rustc-link-lib=python3.10");
    }
}
