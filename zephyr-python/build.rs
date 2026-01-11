//! Build script for zephyr-python crate.
//!
//! Configures linking to Python library on macOS.

fn main() {
    // Configure pyo3 build
    pyo3_build_config::use_pyo3_cfgs();

    // On macOS with Homebrew Python, add library search path
    if std::env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os == "macos")
        .unwrap_or(false)
    {
        // Add library search path for Homebrew Python
        println!(
            "cargo:rustc-link-search=/opt/homebrew/opt/python@3.13/Frameworks/Python.framework/Versions/3.13/lib"
        );
        println!("cargo:rustc-link-lib=python3.13");
    }
}
