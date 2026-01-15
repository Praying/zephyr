//! Python strategy loader using PyO3.
//!
//! This module provides the `PythonLoader` struct for loading Python strategies
//! from script files. It handles Python interpreter initialization, sys.path
//! configuration, and strategy instantiation.
//!
//! # Requirements
//!
//! - Python 3.10+ must be installed
//! - The `python` feature must be enabled in Cargo.toml
//!
//! # Configuration
//!
//! Python strategies are configured via TOML:
//!
//! ```toml
//! [python]
//! venv_dir = "/path/to/venv"
//! python_paths = ["./strategies", "./lib"]
//!
//! [[strategies]]
//! name = "grid_eth"
//! type = "python"
//! path = "./strategies/grid.py"
//! class = "GridStrategy"
//! [strategies.params]
//! grid_size = 10
//! ```
//!
//! # Example Python Strategy
//!
//! ```python
//! class GridStrategy:
//!     def __init__(self, config: str):
//!         import json
//!         self.config = json.loads(config)
//!         self.name = self.config.get("name", "grid_strategy")
//!
//!     def required_subscriptions(self) -> list:
//!         return [("ETH-USDT", "tick")]
//!
//!     def on_init(self, ctx):
//!         pass
//!
//!     def on_tick(self, tick, ctx):
//!         # Trading logic
//!         pass
//!
//!     def on_bar(self, bar, ctx):
//!         pass
//!
//!     def on_stop(self, ctx):
//!         pass
//! ```

use crate::loader::config::PythonConfig;
use crate::loader::python_adapter::{PyStrategyAdapter, extract_subscriptions};
use crate::r#trait::Strategy;
use anyhow::Result;
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

/// Global flag to track if Python interpreter has been initialized.
/// PyO3 requires that the interpreter is only initialized once per process.
static PYTHON_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Errors that can occur during Python strategy loading.
#[derive(Debug, thiserror::Error)]
pub enum PythonLoadError {
    /// Failed to initialize Python interpreter
    #[error("failed to initialize Python interpreter: {0}")]
    InterpreterInit(String),

    /// Failed to read Python script file
    #[error("failed to read Python script '{path}': {source}")]
    ReadScript {
        /// Path to the script that could not be read
        path: String,
        /// The underlying IO error
        #[source]
        source: std::io::Error,
    },

    /// Python syntax error in script
    #[error("Python syntax error in '{path}': {message}")]
    SyntaxError {
        /// Path to the script with syntax error
        path: String,
        /// Error message from Python
        message: String,
    },

    /// Strategy class not found in script
    #[error("class '{class_name}' not found in '{path}'")]
    ClassNotFound {
        /// Path to the script
        path: String,
        /// Name of the class that was not found
        class_name: String,
    },

    /// Failed to instantiate strategy class
    #[error("failed to instantiate '{class_name}': {message}")]
    InstantiationError {
        /// Name of the class that failed to instantiate
        class_name: String,
        /// Error message
        message: String,
    },

    /// Missing Python dependency (ImportError)
    #[error("missing Python dependency '{module}': {message}")]
    MissingDependency {
        /// Name of the missing module
        module: String,
        /// Full error message
        message: String,
    },

    /// General Python error
    #[error("Python error: {0}")]
    PythonError(String),
}

impl From<PyErr> for PythonLoadError {
    fn from(err: PyErr) -> Self {
        Python::with_gil(|py| {
            let err_str = err.to_string();

            // Check if it's an ImportError to provide better error messages
            if err.is_instance_of::<pyo3::exceptions::PyImportError>(py) {
                // Try to extract the module name from the error
                let module = extract_module_from_import_error(&err_str);
                return Self::MissingDependency {
                    module,
                    message: err_str,
                };
            }

            // Check for syntax errors
            if err.is_instance_of::<pyo3::exceptions::PySyntaxError>(py) {
                return Self::SyntaxError {
                    path: String::new(),
                    message: err_str,
                };
            }

            Self::PythonError(err_str)
        })
    }
}

/// Extracts the module name from an ImportError message.
fn extract_module_from_import_error(err_str: &str) -> String {
    // Common patterns:
    // "No module named 'foo'"
    // "cannot import name 'bar' from 'foo'"
    if let Some(start) = err_str.find("No module named '") {
        let rest = &err_str[start + 17..];
        if let Some(end) = rest.find('\'') {
            return rest[..end].to_string();
        }
    }
    if let Some(start) = err_str.find("from '") {
        let rest = &err_str[start + 6..];
        if let Some(end) = rest.find('\'') {
            return rest[..end].to_string();
        }
    }
    "unknown".to_string()
}

/// Python strategy loader.
///
/// Handles loading Python strategies from script files. The loader:
/// 1. Initializes the Python interpreter (once per process)
/// 2. Configures sys.path with user-specified directories
/// 3. Loads Python scripts and instantiates strategy classes
/// 4. Wraps Python strategies in `PyStrategyAdapter`
///
/// # Thread Safety
///
/// The loader is thread-safe. Python's GIL ensures safe concurrent access
/// to the interpreter.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::{PythonLoader, PythonConfig};
/// use serde_json::json;
///
/// let config = PythonConfig {
///     venv_dir: Some("/path/to/venv".into()),
///     python_paths: vec!["./strategies".into()],
/// };
///
/// let loader = PythonLoader::new(&config)?;
/// let strategy = loader.load(
///     "./strategies/grid.py",
///     "GridStrategy",
///     json!({"grid_size": 10}),
/// )?;
/// ```
pub struct PythonLoader {
    /// Configured Python paths (stored for reference)
    _python_paths: Vec<String>,
}

impl PythonLoader {
    /// Creates a new Python loader with the given configuration.
    ///
    /// This initializes the Python interpreter (if not already initialized)
    /// and configures sys.path with the specified directories.
    ///
    /// # Arguments
    ///
    /// * `config` - Python environment configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Python interpreter initialization fails
    /// - sys.path configuration fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = PythonConfig {
    ///     venv_dir: Some("/path/to/venv".into()),
    ///     python_paths: vec!["./strategies".into()],
    /// };
    /// let loader = PythonLoader::new(&config)?;
    /// ```
    pub fn new(config: &PythonConfig) -> Result<Self, PythonLoadError> {
        // Initialize Python interpreter if not already done
        Self::ensure_python_initialized()?;

        // Configure sys.path
        let python_paths = Self::configure_sys_path(config)?;

        info!(
            paths = ?python_paths,
            venv = ?config.venv_dir,
            "Python loader initialized"
        );

        Ok(Self {
            _python_paths: python_paths,
        })
    }

    /// Ensures the Python interpreter is initialized.
    fn ensure_python_initialized() -> Result<(), PythonLoadError> {
        // Only initialize once
        if PYTHON_INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            debug!("Initializing Python interpreter");
            pyo3::prepare_freethreaded_python();
        }
        Ok(())
    }

    /// Configures sys.path with user-specified directories.
    fn configure_sys_path(config: &PythonConfig) -> Result<Vec<String>, PythonLoadError> {
        let mut configured_paths = Vec::new();

        Python::with_gil(|py| {
            let sys = py.import("sys").map_err(|e| {
                PythonLoadError::InterpreterInit(format!("failed to import sys: {e}"))
            })?;

            let path_attr = sys.getattr("path").map_err(|e| {
                PythonLoadError::InterpreterInit(format!("failed to get sys.path: {e}"))
            })?;

            let path: &Bound<'_, PyList> = path_attr.downcast().map_err(|e| {
                PythonLoadError::InterpreterInit(format!("sys.path is not a list: {e}"))
            })?;

            // Add venv site-packages if configured
            if let Some(venv_dir) = &config.venv_dir {
                let site_packages = find_site_packages(venv_dir);
                if let Some(sp) = site_packages {
                    let sp_str = sp.to_string_lossy().to_string();
                    debug!(path = %sp_str, "Adding venv site-packages to sys.path");
                    path.insert(0, &sp_str).map_err(|e| {
                        PythonLoadError::InterpreterInit(format!(
                            "failed to add site-packages to sys.path: {e}"
                        ))
                    })?;
                    configured_paths.push(sp_str);
                } else {
                    warn!(
                        venv = ?venv_dir,
                        "Could not find site-packages in venv directory"
                    );
                }
            }

            // Add user-specified paths
            for user_path in &config.python_paths {
                let path_str = user_path.to_string_lossy().to_string();
                debug!(path = %path_str, "Adding user path to sys.path");
                path.insert(0, &path_str).map_err(|e| {
                    PythonLoadError::InterpreterInit(format!("failed to add path to sys.path: {e}"))
                })?;
                configured_paths.push(path_str);
            }

            Ok(configured_paths)
        })
    }

    /// Loads a Python strategy from a script file.
    ///
    /// This method:
    /// 1. Reads the Python script from the specified path
    /// 2. Adds the script's directory to sys.path (for relative imports)
    /// 3. Loads the module and finds the strategy class
    /// 4. Instantiates the class with the provided configuration
    /// 5. Extracts subscriptions from the instance
    /// 6. Returns a `PyStrategyAdapter` wrapping the Python instance
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Python script file
    /// * `class_name` - Name of the strategy class to instantiate
    /// * `config` - JSON configuration to pass to the constructor
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The script file cannot be read
    /// - The script has syntax errors
    /// - The class is not found in the script
    /// - The class fails to instantiate
    /// - A required Python dependency is missing
    ///
    /// # Example
    ///
    /// ```ignore
    /// let strategy = loader.load(
    ///     "./strategies/grid.py",
    ///     "GridStrategy",
    ///     json!({"grid_size": 10}),
    /// )?;
    /// ```
    pub fn load(
        &self,
        path: &str,
        class_name: &str,
        config: serde_json::Value,
    ) -> Result<Box<dyn Strategy>, PythonLoadError> {
        let script_path = Path::new(path);
        let script_dir = script_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_string_lossy()
            .to_string();

        // Read the script file
        let code =
            std::fs::read_to_string(script_path).map_err(|e| PythonLoadError::ReadScript {
                path: path.to_string(),
                source: e,
            })?;

        debug!(
            path = %path,
            class = %class_name,
            "Loading Python strategy"
        );

        Python::with_gil(|py| {
            // Add script directory to sys.path for relative imports
            let sys = py.import("sys")?;
            let path_attr = sys.getattr("path")?;
            let path_list: &Bound<'_, PyList> = path_attr.downcast().map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                    "sys.path is not a list: {e}"
                ))
            })?;

            // Check if script_dir is already in path to avoid duplicates
            let script_dir_in_path = path_list.iter().any(|p| {
                p.extract::<String>()
                    .map(|s| s == script_dir)
                    .unwrap_or(false)
            });

            if !script_dir_in_path {
                debug!(dir = %script_dir, "Adding script directory to sys.path");
                path_list.insert(0, &script_dir)?;
            }

            // Generate a unique module name to avoid conflicts
            let module_name = format!(
                "zephyr_user_strategy_{}",
                script_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );

            // Convert strings to CString for PyModule::from_code
            let code_cstr = CString::new(code.as_str())
                .map_err(|e| PythonLoadError::PythonError(format!("Invalid code string: {e}")))?;
            let path_cstr = CString::new(path)
                .map_err(|e| PythonLoadError::PythonError(format!("Invalid path string: {e}")))?;
            let module_name_cstr = CString::new(module_name.as_str())
                .map_err(|e| PythonLoadError::PythonError(format!("Invalid module name: {e}")))?;

            // Load the module from code
            let module = PyModule::from_code(py, &code_cstr, &path_cstr, &module_name_cstr)
                .map_err(|e| {
                    // Check for specific error types
                    if e.is_instance_of::<pyo3::exceptions::PySyntaxError>(py) {
                        return PythonLoadError::SyntaxError {
                            path: path.to_string(),
                            message: e.to_string(),
                        };
                    }
                    if e.is_instance_of::<pyo3::exceptions::PyImportError>(py) {
                        let module = extract_module_from_import_error(&e.to_string());
                        return PythonLoadError::MissingDependency {
                            module,
                            message: e.to_string(),
                        };
                    }
                    PythonLoadError::PythonError(e.to_string())
                })?;

            // Get the strategy class
            let class = module
                .getattr(class_name)
                .map_err(|_| PythonLoadError::ClassNotFound {
                    path: path.to_string(),
                    class_name: class_name.to_string(),
                })?;

            // Instantiate with config as JSON string
            let config_str = config.to_string();
            let instance = class.call1((config_str,)).map_err(|e| {
                // Check for ImportError during instantiation
                if e.is_instance_of::<pyo3::exceptions::PyImportError>(py) {
                    let module = extract_module_from_import_error(&e.to_string());
                    return PythonLoadError::MissingDependency {
                        module,
                        message: e.to_string(),
                    };
                }
                PythonLoadError::InstantiationError {
                    class_name: class_name.to_string(),
                    message: e.to_string(),
                }
            })?;

            // Extract subscriptions
            let subscriptions = extract_subscriptions(py, &instance).map_err(|e| {
                PythonLoadError::PythonError(format!("failed to extract subscriptions: {e}"))
            })?;

            info!(
                path = %path,
                class = %class_name,
                subscriptions = subscriptions.len(),
                "Python strategy loaded successfully"
            );

            // Create the adapter
            let adapter =
                PyStrategyAdapter::new(instance.unbind(), class_name.to_string(), subscriptions);

            Ok(Box::new(adapter) as Box<dyn Strategy>)
        })
    }

    /// Reloads a Python strategy from its script file.
    ///
    /// This is used for hot reload functionality. It loads a fresh instance
    /// of the strategy class from the updated script.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Python script file
    /// * `class_name` - Name of the strategy class to instantiate
    /// * `config` - JSON configuration to pass to the constructor
    ///
    /// # Returns
    ///
    /// A new `PyStrategyAdapter` instance with the updated code.
    pub fn reload(
        &self,
        path: &str,
        class_name: &str,
        config: serde_json::Value,
    ) -> Result<Box<dyn Strategy>, PythonLoadError> {
        debug!(path = %path, class = %class_name, "Reloading Python strategy");

        // For reload, we need to invalidate any cached modules
        Python::with_gil(|py| {
            // Try to remove the module from sys.modules to force a fresh load
            let sys = py.import("sys")?;
            let modules = sys.getattr("modules")?;

            let module_name = format!(
                "zephyr_user_strategy_{}",
                Path::new(path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );

            // Remove the module if it exists (ignore errors)
            let _ = modules.call_method1("pop", (module_name,));

            Ok::<_, PyErr>(())
        })?;

        // Now load fresh
        self.load(path, class_name, config)
    }
}

/// Finds the site-packages directory within a virtual environment.
fn find_site_packages(venv_dir: &Path) -> Option<std::path::PathBuf> {
    // Try common locations
    let candidates = [
        // Unix-like systems
        venv_dir
            .join("lib")
            .join("python3.10")
            .join("site-packages"),
        venv_dir
            .join("lib")
            .join("python3.11")
            .join("site-packages"),
        venv_dir
            .join("lib")
            .join("python3.12")
            .join("site-packages"),
        venv_dir
            .join("lib")
            .join("python3.13")
            .join("site-packages"),
        // Windows
        venv_dir.join("Lib").join("site-packages"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    // Try to find any python3.x directory
    let lib_dir = venv_dir.join("lib");
    if lib_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("python3.") {
                    let site_packages = entry.path().join("site-packages");
                    if site_packages.exists() {
                        return Some(site_packages);
                    }
                }
            }
        }
    }

    None
}

/// Creates a default Python loader with no additional paths.
impl Default for PythonLoader {
    fn default() -> Self {
        Self::new(&PythonConfig::default()).expect("Failed to create default PythonLoader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_module_from_import_error() {
        assert_eq!(
            extract_module_from_import_error("No module named 'pandas'"),
            "pandas"
        );
        assert_eq!(
            extract_module_from_import_error("No module named 'numpy.core'"),
            "numpy.core"
        );
        assert_eq!(
            extract_module_from_import_error("cannot import name 'foo' from 'bar'"),
            "bar"
        );
        assert_eq!(
            extract_module_from_import_error("some other error"),
            "unknown"
        );
    }

    #[test]
    fn test_python_load_error_display() {
        let err = PythonLoadError::MissingDependency {
            module: "pandas".to_string(),
            message: "No module named 'pandas'".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("pandas"));
        assert!(msg.contains("missing"));

        let err = PythonLoadError::ClassNotFound {
            path: "./test.py".to_string(),
            class_name: "MyStrategy".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("MyStrategy"));
        assert!(msg.contains("./test.py"));
    }

    #[test]
    fn test_find_site_packages_nonexistent() {
        let result = find_site_packages(Path::new("/nonexistent/path"));
        assert!(result.is_none());
    }
}
