use core::fmt;
use std::iter::Copied;

use shred_core::bitset::Bitset;
use shred_core::symbols::{DEFAULT_ACTION, EOF_ID, FLATTEN_ACTION, OPTION_ACTION, PREPEND_ACTION, SEQUENCE_ACTION, SymbolId};
use shred_core::table::{ActionKind, ParseTable, StateId};

type Span = (usize, usize);

type Item<T> = <T as ItemInitializer>::Item;
type Sequence<T> = <T as ItemInitializer>::Sequence;

#[derive(Debug)]
pub enum AutomatonErrorKind<T: ItemInitializer> {
    StackUnderflow,
    UnexpectedToken { item: T::Item, state_id: StateId }
}

// The following traits are meant to make the automaton very extensible. It should be usable
// with arbitrary heap allocated nodes (for Python objects!) and an interner (which the default uses). 
pub trait ItemInitializer: fmt::Debug {
    type Item: ItemType;
    type Sequence: SequenceType;

    fn create_item(&mut self, span: Span, symbol_id: SymbolId) -> Self::Item;
    fn create_item_from_option(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        item: Option<Self::Item>,
    ) -> Self::Item;
    fn create_sequence_from_vec(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        items: Vec<Self::Item>,
    ) -> Self::Sequence;
}

pub trait ItemType: Default + fmt::Debug + Clone {
    type Init: ItemInitializer;

    fn start(&self, initializer: &Self::Init) -> usize;
    fn end(&self, initializer: &Self::Init) -> usize;
    fn symbol_id(&self, initializer: &Self::Init) -> SymbolId;
    fn updrage_to_sequence(self, initializer: &Self::Init) -> Result<Sequence<Self::Init>, Item<Self::Init>>;
}

pub trait SequenceType: ItemType {
    fn insert(&mut self, initializer: &mut Self::Init, index: usize, value: Item<Self::Init>);
    fn append(&mut self, initializer: &mut Self::Init, items: &mut Vec<Item<Self::Init>>);
    fn push(&mut self, initializer: &mut Self::Init, value: Item<Self::Init>);
    fn to_owned_vec(self, initializer: &mut Self::Init) -> Vec<Item<Self::Init>>;
    fn downgrade_to_item(self, initializer: &Self::Init) -> Item<Self::Init>;
}

pub trait ItemIterator<T: ItemInitializer> {
    fn current(&mut self) -> T::Item;
    fn advance(&mut self);
}

pub struct ItemVecIterator<T, U> {
    iterator: T,
    last_seen: Option<U>,
}

impl<'a, U: Copy> ItemVecIterator<Copied<std::slice::Iter<'a, U>>, U> {
    pub fn new(items: &'a Vec<U>) -> Self {
        ItemVecIterator { iterator: items.iter().copied(), last_seen: None }
    }
}


impl<T, U, V> ItemIterator<T> for ItemVecIterator<U, V>
where
    T: ItemInitializer<Item = V>,
    U: Iterator<Item = V>,
    V: ItemType<Init = T>,
{
    fn current(&mut self) -> T::Item {
        if let Some(current) = self.last_seen.clone() {
            return current;
        }
        if let Some(next) = self.iterator.next() {
            self.last_seen = Some(next);
        }
        self.last_seen.clone().unwrap()
    }

    fn advance(&mut self) {
        if let Some(next) = self.iterator.next() {
            self.last_seen = Some(next);
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct DefaultItem(pub usize);

#[derive(Default, Debug)]
pub enum DefaultItemKind {
    Item,   // XXX: The default item representation has no way of holding very basic things... for example, the content
            // of an identifier token. This was kind of the point but it has the unfortunate side effect of making the
            // resulting parse tree completely and utterly useless. I guess it's slightly useful because someone
            // could index the source code using the span to retrieve the content. A more robust approach, such as
            // requiring ItemIterator to enumarate its output and binding it to this variant, would be more ideal.
            // But that would seemingly imply that one should store all of their tokens in memory, making the entire
            // premise of an iterator questionable. It's reasonable to make the Item variant polymorphic in the pursuit
            // of an actually useful default representation. The extent to which this overenginerred mess of traits and
            // generics will be beneficial for other representations is yet to be determined. I believe it will be plug
            // and play with PyO3 objects.
            // I think I will try to make the grammar automatically generate the concrete AST through an approach like
            // this:
            // nonterminal:
            //      "def" <name:IDENTIFIER> "(" <parameters:function_parameters> ")" "->" <returns:path_expression> => FunctionDefNode
    Sequence(Vec<DefaultItem>),
    Option(Option<DefaultItem>),
    #[default]
    Taken,
}

#[derive(Default, Debug)]
pub struct DefaultItemData {
    span: Span,
    symbol_id: SymbolId,
    kind: DefaultItemKind,
}

#[derive(Debug)]
pub struct DefaultInitializer {
    pub items: Vec<DefaultItemData>,
}

impl DefaultInitializer {
    pub fn new() -> Self {
        DefaultInitializer { items: Vec::new() }
    }
}

impl ItemInitializer for DefaultInitializer {
    type Item = DefaultItem;
    type Sequence = DefaultItem;

    fn create_item(&mut self, span: Span, symbol_id: SymbolId) -> Self::Item {
        let idx = self.items.len();
        self.items.push(DefaultItemData { span, symbol_id, kind: DefaultItemKind::Item });
        DefaultItem(idx)
    }

    fn create_item_from_option(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        item: Option<Self::Item>
    ) -> Self::Item {
        let idx = self.items.len();
        self.items.push(DefaultItemData { span, symbol_id, kind: DefaultItemKind::Option(item) });
        DefaultItem(idx)
    }

    fn create_sequence_from_vec(
        &mut self,
        span: Span,
        symbol_id: SymbolId,
        items: Vec<Self::Item>,
    ) -> Self::Sequence {
        let idx = self.items.len();
        self.items.push(DefaultItemData { span, symbol_id, kind: DefaultItemKind::Sequence(items) });
        DefaultItem(idx)
    }
}

impl ItemType for DefaultItem {
    type Init = DefaultInitializer;

    fn start(&self, initializer: &DefaultInitializer) -> usize {
        initializer.items[self.0].span.0
    }

    fn end(&self, initializer: &DefaultInitializer) -> usize {
        initializer.items[self.0].span.1
    }

    fn symbol_id(&self, initializer: &DefaultInitializer) -> SymbolId {
        initializer.items[self.0].symbol_id
    }

    fn updrage_to_sequence(self, initializer: &DefaultInitializer) -> Result<DefaultItem, DefaultItem> {
        match initializer.items[self.0].kind {
            DefaultItemKind::Sequence(..) => Ok(self),
            _ => Err(self)
        }
    }
}

impl SequenceType for DefaultItem {
    fn insert(
        &mut self,
        initializer: &mut DefaultInitializer,
        index: usize,
        value: DefaultItem,
    ) {
        let DefaultItemData { kind: DefaultItemKind::Sequence(items), .. } =
            &mut initializer.items[self.0] else { panic!("insert on non-sequence item") };
        items.insert(index, value);
    }

    fn append(&mut self, initializer: &mut DefaultInitializer, items: &mut Vec<DefaultItem>) {
        let DefaultItemData { kind: DefaultItemKind::Sequence(these_items), .. } =
            &mut initializer.items[self.0] else { panic!("appen on non-sequence item") };
        these_items.append(items);    
    }

    fn push(&mut self, initializer: &mut DefaultInitializer, value: DefaultItem) {
        let DefaultItemData { kind: DefaultItemKind::Sequence(items), .. } =
            &mut initializer.items[self.0] else { panic!("push on non-sequence item") };
        items.push(value);
    }

    fn to_owned_vec(self, initializer: &mut DefaultInitializer) -> Vec<DefaultItem> {
        let DefaultItemData { kind: DefaultItemKind::Sequence(items), .. } =
            std::mem::take(&mut initializer.items[self.0]) else { panic!("to_owned_vec on non-sequence item") };
        items
    }

    fn downgrade_to_item(self, _initializer: &DefaultInitializer) -> DefaultItem {
        self
    }
}

pub struct ParserAutomaton<'table, 'transformer, T: ItemInitializer, U> {
    table: &'table ParseTable,
    ignored_ends: Bitset<Vec<u64>>,  // Terminal symbol ids to ignore when finding span end of a stack item
    item_initializer: T,
    item_iterator: U,
    transformers: Vec<Box<dyn Fn(&mut T, Span, SymbolId, Vec<T::Item>) -> T::Item + 'transformer>>,
    item_stack: Vec<T::Item>,
    state_stack: Vec<StateId>,
}

impl<'table, 'transformer, T, U, V, W> ParserAutomaton<'table, 'transformer, T, U>
    where
        T: ItemInitializer<Item = V, Sequence = W> + 'transformer,
        U: ItemIterator<T> + 'transformer,
        V: ItemType<Init = T> + 'transformer,
        W: SequenceType<Init = T> + 'transformer,
{
    pub fn new(
        table: &'table ParseTable,
        ignored_ends: Bitset<Vec<u64>>,
        mut item_initializer: T,
        item_iterator: U,
        mut transformers: Vec<Box<dyn Fn(&mut T, Span, SymbolId, Vec<T::Item>) -> T::Item + 'transformer>>,
        entry_state: StateId,
    ) -> Self {
        transformers.insert(DEFAULT_ACTION.0 as usize, Box::new(Self::default_action));
        transformers.insert(SEQUENCE_ACTION.0 as usize, Box::new(Self::sequence_action));
        transformers.insert(PREPEND_ACTION.0 as usize, Box::new(Self::prepend_action));
        transformers.insert(FLATTEN_ACTION.0 as usize, Box::new(Self::flatten_action));
        transformers.insert(OPTION_ACTION.0 as usize, Box::new(Self::option_action));

        ParserAutomaton {
            table,
            ignored_ends,
            item_iterator,
            transformers,
            item_stack: vec![item_initializer.create_item((0, 0), EOF_ID)],
            state_stack: vec![entry_state],
            item_initializer,
        }
    }

    fn default_action(initializer: &mut T, span: Span, symbol_id: SymbolId, mut items: Vec<T::Item>) -> T::Item {
        if items.len() == 1 {
            items.remove(0)
        }
        else {
            initializer.create_sequence_from_vec(span, symbol_id, items).downgrade_to_item(initializer)
        }
    }

    fn sequence_action(initializer: &mut T, span: Span, symbol_id: SymbolId, items: Vec<T::Item>) -> T::Item {
        initializer.create_sequence_from_vec(span, symbol_id, items).downgrade_to_item(initializer)
    }

    fn prepend_action(initializer: &mut T, _span: Span, _symbol_id: SymbolId, mut items: Vec<T::Item>) -> T::Item {
        if items.len() != 2 {
            panic!("Prepend must be called on two items, found {}", items.len());
        }

        let first_item = items.remove(0);
        let second_item = items.remove(0);
        match second_item.updrage_to_sequence(initializer) {
            Ok(mut item) => {
                item.insert(initializer, 0, first_item);
                return item.downgrade_to_item(initializer);
            }
            _ => {
                panic!("The second item of the prepend intrinsic must be a sequence");
            }
        }
    }

    fn flatten_action(initializer: &mut T, span: Span, symbol_id: SymbolId, items: Vec<T::Item>) -> T::Item {
        let mut result: Vec<T::Item> = Vec::new();
        for item in items {
            match item.updrage_to_sequence(initializer) {
                Ok(item) => {
                    result.append(&mut item.to_owned_vec(initializer));
                }
                Err(item) => result.push(item),
            }
        }
        initializer.create_sequence_from_vec(span, symbol_id, result).downgrade_to_item(initializer)
    }

    fn option_action(initializer: &mut T, span: Span, symbol_id: SymbolId, mut items: Vec<T::Item>) -> T::Item {
        if items.len() == 0 {
            initializer.create_item_from_option(span, symbol_id, None)
        }
        else {
            assert_eq!(items.len(), 2, "The option intrinsic can only take two items from the stack");
            initializer.create_item_from_option(span, symbol_id, Some(items.remove(0)))
        }
    }

    pub fn run(&mut self) -> Result<T::Item, AutomatonErrorKind<T>> {
        loop {
            if self.state_stack.len() == 0 {
                return Err(AutomatonErrorKind::StackUnderflow);
            }
            let current_state = self.state_stack[self.state_stack.len() - 1];
            let current_item = self.item_iterator.current();
            match self.table.action(current_state, current_item.symbol_id(&self.item_initializer)) {
                ActionKind::Reject => {
                    return Err(AutomatonErrorKind::UnexpectedToken { item: current_item, state_id: current_state });
                }
                ActionKind::Shift(next_state_id) => {
                    self.item_iterator.advance();
                    self.item_stack.push(current_item);
                    self.state_stack.push(next_state_id);
                }
                ActionKind::Reduce(production_id) => {
                    let production = self.table.interned_symbols.production(production_id);

                    let mut items: Vec<T::Item> = Vec::with_capacity(production.rhs.len());

                    let span = if production.rhs.len() > 0 {
                        let end = self.item_stack.iter().rev().find(|item| {
                            let symbol = self.table.interned_symbols.symbol(item.symbol_id(&self.item_initializer));
                                symbol.is_nonterminal() || !self.ignored_ends.has(self.table.interned_symbols.terminal_index(symbol.id))
                            })
                            .map(|item| item.end(&self.item_initializer));

                        items.extend(self.item_stack.drain(self.item_stack.len() - production.rhs.len()..));
                        self.state_stack.drain(self.state_stack.len() - production.rhs.len()..);

                        let start = items[0].start(&self.item_initializer);
                        (start, end.unwrap_or(start))
                    }
                    else {
                        if self.item_stack.len() == 0 {
                            return Err(AutomatonErrorKind::StackUnderflow);
                        }

                        let item = &self.item_stack[self.item_stack.len() - 1];
                        (item.end(&self.item_initializer), item.end(&self.item_initializer))
                    };

                    let transformer = &self.transformers[production.action.0 as usize];
                    let item = transformer(
                        &mut self.item_initializer,
                        span,
                        production.lhs_id,
                        items,
                    );
                    self.item_stack.push(item);

                    if self.state_stack.len() == 0 {
                        return Err(AutomatonErrorKind::StackUnderflow);
                    }
                    let current_state = self.state_stack[self.state_stack.len() - 1];
                    match self.table.goto(current_state, production.lhs_id) {
                        Some(next_state_id) => self.state_stack.push(next_state_id),
                        None => {
                            let symbol = self.table.interned_symbols.symbol(production.lhs_id);
                            panic!("Parser found no GOTO for {} in {}", symbol.name, current_state.0);
                        }
                    }
                }
                ActionKind::Accept(production_id) => {
                    let span = (0, current_item.end(&self.item_initializer));

                    let production = self.table.interned_symbols.production(production_id);
                    let items = self.item_stack.drain(
                            self.item_stack.len() - production.rhs.len()..
                        )
                        .collect();
                    self.state_stack.drain(production.rhs.len()..);

                    debug_assert_eq!(self.item_stack.len(), 1, "Accept action left excess data on the item stack");
                    debug_assert_eq!(self.state_stack.len(), 1, "Accept action left excess data on the state stack");

                    let transformer = &self.transformers[production.action.0 as usize];
                    let item = transformer(
                        &mut self.item_initializer,
                        span,
                        production.lhs_id,
                        items,
                    );
                    return Ok(item);
                }
            }
        }
    }
}
