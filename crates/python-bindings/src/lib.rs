mod automaton;
mod bitset;
mod table;

use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
mod python_bindings {
    #[pymodule_export]
    use super::automaton::{UnexpectedTokenError, BaseItem, SequenceItem, OptionItem, PyParserAutomaton};

    #[pymodule_export]
    use super::bitset::{PyBitset};

    #[pymodule_export]
    use super::table::{PyParseTable};
}
