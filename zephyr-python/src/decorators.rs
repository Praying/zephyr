//! Python strategy decorators.
//!
//! This module provides decorators for defining Python strategies
//! that integrate with the Zephyr trading engine.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

/// Strategy metadata extracted from decorated classes.
#[pyclass(name = "StrategyMeta", module = "zephyr_py")]
#[derive(Debug)]
pub struct PyStrategyMeta {
    /// Strategy name
    #[pyo3(get)]
    pub name: String,
    /// Strategy type (cta or hft)
    #[pyo3(get)]
    pub strategy_type: String,
    /// Trading symbols
    #[pyo3(get)]
    pub symbols: Vec<String>,
    /// Strategy parameters as a HashMap
    params_map: HashMap<String, PyObject>,
}

#[pymethods]
impl PyStrategyMeta {
    /// Creates a new strategy metadata.
    #[new]
    #[pyo3(signature = (name, strategy_type, symbols=vec![], params=None))]
    pub fn new(
        py: Python<'_>,
        name: String,
        strategy_type: String,
        symbols: Vec<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let params_map = if let Some(dict) = params {
            let mut map = HashMap::new();
            for (key, value) in dict.iter() {
                let key_str: String = key.extract()?;
                map.insert(key_str, value.into_pyobject(py)?.unbind());
            }
            map
        } else {
            HashMap::new()
        };

        Ok(Self {
            name,
            strategy_type,
            symbols,
            params_map,
        })
    }

    /// Gets the parameters as a Python dict.
    #[getter]
    pub fn params(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.params_map {
            dict.set_item(key, value.bind(py))?;
        }
        Ok(dict.unbind())
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        format!(
            "StrategyMeta(name='{}', type='{}', symbols={:?})",
            self.name, self.strategy_type, self.symbols
        )
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        self.__str__()
    }
}

/// Internal decorator state for CTA strategies.
#[pyclass(name = "_CtaDecorator", module = "zephyr_py")]
pub struct CtaDecorator {
    name: String,
    symbols: Vec<String>,
    params: HashMap<String, PyObject>,
}

#[pymethods]
impl CtaDecorator {
    #[new]
    #[pyo3(signature = (name, symbols, params))]
    fn new(
        py: Python<'_>,
        name: String,
        symbols: Vec<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let params_map = if let Some(dict) = params {
            let mut map = HashMap::new();
            for (key, value) in dict.iter() {
                let key_str: String = key.extract()?;
                map.insert(key_str, value.into_pyobject(py)?.unbind());
            }
            map
        } else {
            HashMap::new()
        };

        Ok(Self {
            name,
            symbols,
            params: params_map,
        })
    }

    fn __call__(&self, py: Python<'_>, cls: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        // Create the metadata
        let params_dict = PyDict::new(py);
        for (key, value) in &self.params {
            params_dict.set_item(key, value.bind(py))?;
        }

        let meta = PyStrategyMeta::new(
            py,
            self.name.clone(),
            "cta".to_string(),
            self.symbols.clone(),
            Some(&params_dict),
        )?;

        // Set the metadata on the class
        cls.setattr("_zephyr_meta", Py::new(py, meta)?)?;

        // Return the class unchanged
        Ok(cls.clone().unbind())
    }
}

/// Internal decorator state for HFT strategies.
#[pyclass(name = "_HftDecorator", module = "zephyr_py")]
pub struct HftDecorator {
    name: String,
    symbols: Vec<String>,
    params: HashMap<String, PyObject>,
}

#[pymethods]
impl HftDecorator {
    #[new]
    #[pyo3(signature = (name, symbols, params))]
    fn new(
        py: Python<'_>,
        name: String,
        symbols: Vec<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let params_map = if let Some(dict) = params {
            let mut map = HashMap::new();
            for (key, value) in dict.iter() {
                let key_str: String = key.extract()?;
                map.insert(key_str, value.into_pyobject(py)?.unbind());
            }
            map
        } else {
            HashMap::new()
        };

        Ok(Self {
            name,
            symbols,
            params: params_map,
        })
    }

    fn __call__(&self, py: Python<'_>, cls: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        // Create the metadata
        let params_dict = PyDict::new(py);
        for (key, value) in &self.params {
            params_dict.set_item(key, value.bind(py))?;
        }

        let meta = PyStrategyMeta::new(
            py,
            self.name.clone(),
            "hft".to_string(),
            self.symbols.clone(),
            Some(&params_dict),
        )?;

        // Set the metadata on the class
        cls.setattr("_zephyr_meta", Py::new(py, meta)?)?;

        // Return the class unchanged
        Ok(cls.clone().unbind())
    }
}

/// CTA strategy decorator.
///
/// Marks a class as a CTA strategy and registers its metadata.
///
/// # Examples
///
/// ```python
/// @cta_strategy(name="my_strategy", symbols=["BTC-USDT"])
/// class MyStrategy:
///     def on_init(self, ctx: CtaStrategyContext):
///         pass
///     
///     def on_bar(self, ctx: CtaStrategyContext, bar: KlineData):
///         pass
/// ```
#[pyfunction]
#[pyo3(signature = (name, symbols=vec![], **kwargs))]
pub fn cta_strategy(
    py: Python<'_>,
    name: String,
    symbols: Vec<String>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<CtaDecorator> {
    CtaDecorator::new(py, name, symbols, kwargs)
}

/// HFT strategy decorator.
///
/// Marks a class as an HFT strategy and registers its metadata.
///
/// # Examples
///
/// ```python
/// @hft_strategy(name="my_hft", symbols=["BTC-USDT"])
/// class MyHftStrategy:
///     def on_init(self, ctx: HftStrategyContext):
///         pass
///     
///     def on_tick(self, ctx: HftStrategyContext, tick: TickData):
///         pass
/// ```
#[pyfunction]
#[pyo3(signature = (name, symbols=vec![], **kwargs))]
pub fn hft_strategy(
    py: Python<'_>,
    name: String,
    symbols: Vec<String>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<HftDecorator> {
    HftDecorator::new(py, name, symbols, kwargs)
}

/// Gets the strategy metadata from a decorated class.
///
/// # Examples
///
/// ```python
/// @cta_strategy(name="test", symbols=["BTC-USDT"])
/// class TestStrategy:
///     pass
///
/// meta = get_strategy_meta(TestStrategy)
/// print(meta.name)  # "test"
/// ```
#[pyfunction]
#[allow(clippy::unnecessary_wraps)]
pub fn get_strategy_meta(cls: &Bound<'_, PyAny>) -> PyResult<Option<PyObject>> {
    match cls.getattr("_zephyr_meta") {
        Ok(meta) => Ok(Some(meta.unbind())),
        Err(_) => Ok(None),
    }
}

/// Registers decorator functions with the Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStrategyMeta>()?;
    m.add_class::<CtaDecorator>()?;
    m.add_class::<HftDecorator>()?;
    m.add_function(wrap_pyfunction!(cta_strategy, m)?)?;
    m.add_function(wrap_pyfunction!(hft_strategy, m)?)?;
    m.add_function(wrap_pyfunction!(get_strategy_meta, m)?)?;
    Ok(())
}
