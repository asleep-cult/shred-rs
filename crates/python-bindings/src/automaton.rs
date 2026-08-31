use pyo3::{prelude::*, types::{self, PyList}};
use shred_core::symbols::SymbolId;
use shred_automaton::automaton::{ItemInitializer, ParserAutomaton, Item, Sequence, Span, ItemType, SequenceType};

struct PyItemInitializer<'py> {
    py: Python<'py>,
    new_item: Bound<'py, types::PyFunction>,
    new_option: Bound<'py, types::PyFunction>,
    new_sequence: Bound<'py, types::PyFunction>,
}

#[derive(Debug, Clone)]
struct PyItem<'py>(Bound<'py, PyAny>);

impl<'py> Default for PyItem<'py> {
    fn default() -> Self {
        todo!()
    }
}

#[derive(Debug, Clone)]
struct PySequence<'py>(Bound<'py, PyAny>, Bound<'py, types::PyList>);

impl<'py> ItemInitializer for PyItemInitializer<'py> {
    type Item = PyItem<'py>;
    type Sequence = PySequence<'py>;

    fn create_item(&mut self, span: Span, symbol_id: SymbolId) -> Self::Item {
        PyItem(self.new_item.call((span, symbol_id.0), None).expect("Failed to create a new item"))
    }

    fn create_item_from_option(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        item: Option<Self::Item>,
    ) -> Self::Item
    {
        PyItem(self.new_option.call((span, symbol_id.0, item.map(|item| item.0)), None)
            .expect("Failed to create new option"))
    }

    fn create_sequence_from_vec(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        items: Vec<Self::Item>,
    ) -> Self::Sequence
    {
        let list = PyList::new(self.py, items.iter().map(|item| &item.0))
            .expect("Failed to create new list");
        let sequence = self.new_sequence.call((span, symbol_id.0, &list), None)
            .expect("Failed to create new sequence");
        PySequence(sequence, list)
    }
} 

impl<'py> ItemType for PyItem<'py> {
    type Init = PyItemInitializer<'py>;

    fn start(&self, initializer: &Self::Init) -> usize {
        self.0.getattr("start").unwrap().extract().unwrap()
    }

    fn end(&self, initializer: &Self::Init) -> usize {
        self.0.getattr("end").unwrap().extract().unwrap()
    }

    fn symbol_id(&self, initializer: &Self::Init) -> SymbolId {
        SymbolId(self.0.getattr("symbol_id").unwrap().extract().unwrap())
    }

    fn updrage_to_sequence(self, initializer: &Self::Init) -> Result<Sequence<Self::Init>, Item<Self::Init>> {
        match self.0.getattr("items") {
            Ok(result) => {
                if let Ok(list) = result.cast_into::<types::PyList>() {
                    Ok(PySequence(self.0, list))
                }
                else {
                    Err(self)
                }
            }
            _ => Err(self)
        }
    }
}

struct PyParserAutomaton<'py> {
    automaton: ParserAutomaton<'py, 'py, >
}
