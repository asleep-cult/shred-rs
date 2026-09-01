use std::iter::once;
use pyo3::{IntoPyObjectExt, exceptions::{PyException, PyRuntimeError, PyValueError}, prelude::*, types::{self, PyList, PyTuple}};
use crate::table::PyParseTable;
use crate::bitset::PyBitset;
use shred_core::symbols::{SymbolId, INTRINSIC_BOUNDARY};
use shred_core::table::StateId;
use shred_automaton::automaton::{
    AutomatonErrorKind, InitializerResult, ItemInitializer, ItemIterator, ItemType, ParserAutomaton, Span, TransformFn
};

// What we learned: Switching between Python and Rust repeatedly for token scanning and node creation
// is very inefficient and offers no performance benefit over vanilla Python. As a result, Rust must
// take on the responsibility of defining the shape of AST nodes and also constructing them.
// Since the AST is constructed from a list of stack items, it is desireable to store the relevant
// data directly from that vector and simply map names to indices.

pub struct PyItemInitializer<'py> {
    py: Python<'py>,
}

#[pyclass(extends=PyException)]
pub struct UnexpectedTokenError {
    #[pyo3(get)]
    item: Py<BaseItem>,
    #[pyo3(get)]
    state_id: u16,
}

#[pymethods]
impl UnexpectedTokenError {
    #[new]
    fn new(item: Py<BaseItem>, state_id: u16) -> Self {
        UnexpectedTokenError { item, state_id }
    }
}

#[pyclass(subclass)]
pub struct BaseItem {
    #[pyo3(get)]
    span: Span,
    #[pyo3(get)]
    symbol_id: u16,
}

#[pymethods]
impl BaseItem {
    #[new]
    fn new(span: Span, symbol_id: u16) -> Self {
        BaseItem { span, symbol_id }
    }
}

#[pyclass(extends=BaseItem)]
pub struct SequenceItem {
    #[pyo3(get)]
    items: Py<types::PyList>,
}

#[pymethods]
impl SequenceItem {
    #[new]
    fn new(py: Python<'_>, span: Span, symbol_id: u16) -> PyClassInitializer<Self> {
        PyClassInitializer::from(BaseItem::new(span, symbol_id))
            .add_subclass(SequenceItem { items: PyList::empty(py).into() })
    }
}

#[pyclass(extends=BaseItem)]
pub struct OptionItem {
    #[pyo3(get)]
    item: Py<types::PyAny>,
}

#[pymethods]
impl OptionItem {
    #[new]
    fn new(span: Span, symbol_id: u16, item: Py<PyAny>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(BaseItem::new(span, symbol_id))
            .add_subclass(OptionItem { item })
    }
}

#[derive(Debug, Clone)]
pub struct PyItem<'py>(Bound<'py, BaseItem>);

impl<'py> ItemInitializer for PyItemInitializer<'py> {
    type Item = PyItem<'py>;
    type Error = PyErr;

    fn create_item(&mut self, span: Span, symbol_id: SymbolId) -> InitializerResult<PyItem<'py>, Self> {
        Ok(PyItem(BaseItem { span, symbol_id: symbol_id.0 }.into_pyobject(self.py)?))
    }

    fn create_item_from_option(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        item: Option<Self::Item>,
    ) -> InitializerResult<PyItem<'py>, Self>
    {
        let result = match item {
            Some(item) => item.0.into_any(),
            None => types::PyNone::get(self.py).as_any().into(),
        };
        let obj = Py::new(self.py, OptionItem::new(span, symbol_id.0, result.into()))?;
        Ok(PyItem(obj.into_bound(self.py).cast_into::<BaseItem>()?))
    }

    fn create_sequence_from_vec(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        items: Vec<Self::Item>,
    ) -> InitializerResult<PyItem<'py>, Self>
    {
        let obj = Py::new(self.py, SequenceItem::new(self.py, span, symbol_id.0))?;
        {
            let obj_ref = obj.borrow(self.py);
            let obj_items = obj_ref.items.bind(self.py);
            for item in items {
                obj_items.append(item.0)?;
            }
        }

        Ok(PyItem(obj.into_bound(self.py).cast_into::<BaseItem>()?))
    }
} 

impl<'py> ItemType for PyItem<'py> {
    type Init = PyItemInitializer<'py>;

    fn start(&self, _initializer: &Self::Init) -> usize {
        self.0.borrow().span.0
    }

    fn end(&self, _initializer: &Self::Init) -> usize {
        self.0.borrow().span.0
    }

    fn symbol_id(&self, _initializer: &Self::Init) -> SymbolId {
        SymbolId(self.0.borrow().symbol_id)
    }

    fn is_sequence(&self, _initializer: &Self::Init) -> bool {
        self.0.is_instance_of::<SequenceItem>()
    }

    fn insert(
        &mut self,
        initializer: &mut Self::Init,
        index: usize,
        value: <Self::Init as ItemInitializer>::Item,
    ) -> InitializerResult<(), Self::Init>
    {
        let obj_ref = self.0.cast::<SequenceItem>()?.borrow();
        let obj_items = obj_ref.items.bind(initializer.py);
        obj_items.insert(index, value.0)?;
        Ok(())
    }

    fn append(
        &mut self,
        initializer: &mut Self::Init,
        item: PyItem<'py>,
    ) -> InitializerResult<(), Self::Init>
    {
        let obj_ref = self.0.cast::<SequenceItem>()?.borrow();
        let obj_items = obj_ref.items.bind(initializer.py);

        let itemobj_ref = item.0.cast::<SequenceItem>()?.borrow();
        let itemobj_items = itemobj_ref.items.bind(initializer.py);
        for item in itemobj_items {
            obj_items.append(item)?;
        }

        Ok(())
    }

    fn push(
        &mut self,
        initializer: &mut Self::Init,
        value: <Self::Init as ItemInitializer>::Item,
    ) -> InitializerResult<(), Self::Init>
    {
        let obj_ref = self.0.cast::<SequenceItem>()?.borrow();
        let obj_items = obj_ref.items.bind(initializer.py);
        obj_items.append(value.0)?;
        Ok(())
    }
}

pub struct PyItemIterator<'py> {
    pub current_fn: Bound<'py, types::PyAny>,
    pub advance_fn: Bound<'py, types::PyAny>,
}

impl<'py> ItemIterator<PyItemInitializer<'py>> for PyItemIterator<'py> {
    fn current(&mut self, _initializer: &PyItemInitializer<'py>) -> InitializerResult<PyItem<'py>, PyItemInitializer<'py>> {
        let item = self.current_fn.call0()?;
        Ok(PyItem(item.cast_into::<BaseItem>()?.into()))
    }

    fn advance(&mut self, _initializer: &PyItemInitializer<'py>) -> InitializerResult<(), PyItemInitializer<'py>> {
        self.advance_fn.call0()?;
        Ok(())
    }
}

#[pyclass]
pub struct PyParserAutomaton {
    #[pyo3(get)]
    table: Py<PyParseTable>,
    #[pyo3(get)]
    ignored_ends: Py<PyBitset>,
    #[pyo3(get)]
    transformers: Py<types::PyDict>,
    #[pyo3(get)]
    current: Py<types::PyAny>,
    #[pyo3(get)]
    advance: Py<types::PyAny>,
}

#[pymethods]
impl PyParserAutomaton {
    #[new]
    fn new(
        table: Py<PyParseTable>,
        ignored_ends: Py<PyBitset>,
        transformers: Py<types::PyDict>,
        current: Py<types::PyAny>,
        advance: Py<types::PyAny>,
    ) -> Self {
        PyParserAutomaton { table, ignored_ends, transformers, current, advance }
    }

    fn run<'py>(&self, py: Python<'py>, entry_state: u16) -> PyResult<Bound<'py, BaseItem>> {
        let mut transformers: Vec<TransformFn<'py, PyItemInitializer>> = Vec::new();
        let table = &self.table.borrow(py).table;

        let transform_map = self.transformers.bind(py);
        for (idx, transformer) in table.interned_symbols.actions.iter().enumerate() {
            if idx as u16 > INTRINSIC_BOUNDARY {
                let Ok(Some(element)) = transform_map.get_item(transformer) else {
                    return Err(PyValueError::new_err(format!("Failed to find transformer: {}", transformer)));
                };

                let py_func = element.clone();
                transformers.push(Box::new(move |init, span, _symbol_id, items| {
                    let iter = once(span.into_bound_py_any(init.py)?)
                        .chain(items.into_iter().map(|item| item.0.into_any()))
                        .collect::<Vec<Bound<'_, PyAny>>>();

                    let args = PyTuple::new(init.py, iter)?;
                    let result = py_func.call1(args)?.cast_into::<BaseItem>()?;
                    Ok(PyItem(result))
                }));
            }
        }

        let result = ParserAutomaton::new(
            table,
            self.ignored_ends.borrow(py).bitset.clone(),
            PyItemInitializer { py },
            PyItemIterator { current_fn: self.current.bind(py).into(), advance_fn: self.advance.bind(py).into() },
            transformers,
            StateId(entry_state),
        ).and_then(|mut auto| auto.run());

        match result {
            Ok(item) => Ok(item.0),
            Err(AutomatonErrorKind::StackUnderflow) => Err(
                PyRuntimeError::new_err("Automaton attempted to access value from empty stack")
            ),
            Err(AutomatonErrorKind::UnexpectedToken { item, state_id }) =>
                Err(PyErr::new::<UnexpectedTokenError, _>((item.0.unbind(), state_id.0))),
            Err(AutomatonErrorKind::InitializerError(err)) => Err(err),
        }
    }
}
