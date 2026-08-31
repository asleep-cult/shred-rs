use pyo3::prelude::*;

use pyo3::types;
use shred_core::bitset::Bitset;

#[pyclass]
struct PyBitset {
    bitset: Bitset<Vec<u64>>,
}

#[pymethods]
impl PyBitset {
    #[new]
    pub fn new(number: &Bound<'_, PyAny>) -> PyResult<Self> {
        let nbits: usize = number.call_method0("bit_length")?.extract()?;
        let nbytes = nbits.div_ceil(8);

        if nbytes == 0 {
            return Ok(PyBitset { bitset: Bitset(vec![0]) });
        }

        let py = number.py();
        let kwargs = types::PyDict::new(py);
        kwargs.set_item("byteorder", "little");

        let bytes: Vec<u8> = number.call_method("to_bytes", (nbytes,), Some(&kwargs))?
            .extract()?;

        let bitset_data: Vec<u64> = bytes.chunks(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        Ok(PyBitset { bitset: Bitset(bitset_data) })
    }

    pub fn add(&mut self, index: usize) -> bool {
        self.bitset.add(index)
    }

    pub fn has(&self, index: usize) -> bool {
        self.bitset.has(index)
    }

    pub fn inplace_union(&mut self, other: &PyBitset) -> bool {
        self.bitset.inplace_union(&other.bitset)
    }

    pub fn is_superset(&self, other: &PyBitset) -> bool {
        self.bitset.is_superset(&other.bitset)
    }

    pub fn clear(&mut self) {
        self.bitset.clear()
    }
}
