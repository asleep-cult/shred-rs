use shred_core::bitset::Bitset;
use shred_core::symbols::{DEFAULT_ACTION, EOF_ID, FLATTEN_ACTION, OPTION_ACTION, PREPEND_ACTION, SEQUENCE_ACTION, SymbolId};
use shred_core::table::{ActionKind, ParseTable, StateId};

type Span = (usize, usize);
type TransformVec<T> = Vec<Box<dyn Fn(Span, Vec<StackItem<T>>) -> Box<PayloadKind<T>>>>;

pub enum AutomatonErrorKind<T> {
    StackUnderflow,
    UnexpectedToken { item: StackItem<T>, state_id: StateId }
}

pub trait SymbolIterator<T> {
    fn current(&mut self) -> StackItem<T>;
    fn advance(&mut self);
}

#[derive(Debug)]
pub enum PayloadKind<T> {
    Sequence(Vec<StackItem<T>>),
    Option(Option<StackItem<T>>),
    Bare(T),
}

#[derive(Debug)]
pub struct StackItem<T> {
    symbol_id: SymbolId,
    span: Span,
    payload: Box<PayloadKind<T>>,
}

pub struct ParserAutomaton<'a, T, U> {
    table: &'a ParseTable,
    ignored_ends: Bitset<Vec<u64>>,  // Terminal symbol ids to ignore when finding span end of StackItem
    item_iterator: U,
    transformers: TransformVec<T>,
    item_stack: Vec<StackItem<T>>,
    state_stack: Vec<StateId>,
}

impl<'a, T: 'static, U: SymbolIterator<T> + 'static> ParserAutomaton<'a, T, U> {
    pub fn new(
        table: &'a ParseTable,
        ignored_ends: Bitset<Vec<u64>>,
        item_iterator: U,
        mut transformers: TransformVec<T>,
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
            item_stack: vec![StackItem {
                symbol_id: EOF_ID,
                span: (0, 0),
                payload: Box::new(PayloadKind::Option(None)),
            }],
            state_stack: vec![entry_state]
        }
    }

    fn default_action(span: Span, mut items: Vec<StackItem<T>>) -> Box<PayloadKind<T>> {
        if items.len() == 1 {
            items.remove(0).payload
        }
        else {
            Box::new(PayloadKind::Sequence(items))
        }
    }

    fn sequence_action(span: Span, items: Vec<StackItem<T>>) -> Box<PayloadKind<T>> {
        Box::new(PayloadKind::Sequence(items))
    }

    fn prepend_action(span: Span, mut items: Vec<StackItem<T>>) -> Box<PayloadKind<T>> {
        if items.len() != 2 {
            panic!("Prepend must be called on two items, found {}", items.len());
        }

        let first_item = items.remove(0);
        let mut second_item = items.remove(0);
        match *second_item.payload {
            PayloadKind::Sequence(ref mut inner_items) => {
                inner_items.insert(0, first_item);
            }
            _ => {
                panic!("The second item of the prepend intrinsic must be a sequence");
            }
        }
        second_item.payload
    }

    fn flatten_action(span: Span, items: Vec<StackItem<T>>) -> Box<PayloadKind<T>> {
        let mut result: Vec<StackItem<T>> = Vec::new();
        for mut item in items {
            match *item.payload {
                PayloadKind::Sequence(ref mut inner_items) => {
                    result.append(inner_items);
                }
                _ => result.push(item),
            }
        }
        Box::new(PayloadKind::Sequence(result))
    }

    fn option_action(span: Span, mut items: Vec<StackItem<T>>) -> Box<PayloadKind<T>> {
        let paylaoad = if items.len() == 0 {
            PayloadKind::Option(None)
        }
        else {
            assert_eq!(items.len(), 2, "The option intrinsic can only take two items from the stack");
            PayloadKind::Option(Some(items.remove(0)))
        };
        Box::new(paylaoad)
    }

    pub fn run(mut self) -> Result<StackItem<T>, AutomatonErrorKind<T>> {
        loop {
            if self.state_stack.len() == 0 {
                return Err(AutomatonErrorKind::StackUnderflow);
            }
            let current_state = self.state_stack[self.state_stack.len() - 1];
            let current_item = self.item_iterator.current();

            match self.table.action(current_state, current_item.symbol_id) {
                ActionKind::Reject => {
                    return Err(AutomatonErrorKind::UnexpectedToken { item: current_item, state_id: current_state });
                }
                ActionKind::Shift(next_state_id) => {
                    self.item_stack.push(current_item);
                    self.state_stack.push(next_state_id);
                }
                ActionKind::Reduce(production_id) => {
                    let production = self.table.interned_symbols.production(production_id);

                    let mut items: Vec<StackItem<T>> = Vec::with_capacity(production.rhs.len());

                    let span = if production.rhs.len() > 0 {
                        let end = self.item_stack.iter().rev().find(|item| {
                            let symbol = self.table.interned_symbols.symbol(item.symbol_id);
                                symbol.is_nonterminal() || !self.ignored_ends.has(self.table.interned_symbols.terminal_index(symbol.id))
                            })
                            .map(|item| item.span.1)
                            .unwrap_or(items[0].span.0);

                        items.extend(self.item_stack.drain(production.rhs.len()..));
                        self.state_stack.drain(production.rhs.len()..);

                        (items[0].span.0, end)
                    }
                    else {
                        if self.item_stack.len() == 0 {
                            return Err(AutomatonErrorKind::StackUnderflow);
                        }

                        let item = &self.item_stack[self.item_stack.len() - 1];
                        (item.span.1, item.span.1)
                    };

                    let transformer = &self.transformers[production.action.0 as usize];
                    let payload = transformer(span, items);
                    self.item_stack.push(StackItem { symbol_id: production.lhs_id, span, payload });

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
                    let span = (0, current_item.span.1);

                    let production = self.table.interned_symbols.production(production_id);
                    let items = self.item_stack.drain(production.rhs.len()..).collect();
                    self.state_stack.drain(production.rhs.len()..);

                    debug_assert_eq!(self.item_stack.len(), 1, "Accept action left excess data on the item stack");
                    debug_assert_eq!(self.state_stack.len(), 1, "Accept action left excess data on the state stack");

                    let transformer = &self.transformers[production.action.0 as usize];
                    let payload = transformer(span, items);
                    return Ok(StackItem { symbol_id: production.lhs_id, span, payload });
                }
            }
        }
    }
}
