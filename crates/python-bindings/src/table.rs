use pyo3::prelude::*;
use shred_core::table::ParseTable;
use shred_generator::generator::TableGenerator;

#[pyclass]
pub struct PyParseTable {
    pub(crate) table: ParseTable,
}

#[pymethods]
impl PyParseTable {
    #[classmethod]
    fn from_grammar(_cls: &Bound<'_, pyo3::types::PyType>, source: &str) -> Self {
        let table = TableGenerator::generate_from_grammar(source);
        PyParseTable { table }
    }
}
