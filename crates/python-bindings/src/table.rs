use pyo3::prelude::*;
use shred_core::table::ParseTable;
use shred_generator::generator::TableGenerator;

#[pyclass]
struct PyParseTable {
    table: ParseTable,
}

#[pymethods]
impl PyParseTable {
    #[classmethod]
    fn from_grammar(cls: &Bound<'_, pyo3::types::PyType>, source: &str) -> Self {
        let table = TableGenerator::generate_from_grammar(source);
        PyParseTable { table }
    }
}
