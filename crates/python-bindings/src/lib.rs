mod automaton;
mod bitset;
mod table;

use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
mod python_bindings {
    use super::*;

    /// Formats the sum of two numbers as string.
    #[pyfunction]
    fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
        bitset::
        Ok((a + b).to_string())
    }
}
