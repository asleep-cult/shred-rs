use shred_generator::generator::TableGenerator;
use shred_core::bitset::Bitset;
use shred_automaton::automaton::{DefaultInitializer, ItemInitializer, ItemVecIterator, ParserAutomaton};

fn main() {
    let source = std::fs::read_to_string("./grammar_test").unwrap();
    let table = TableGenerator::generate_from_grammar(&source);

    let mut initializer = DefaultInitializer::new();
    let items = vec![
        initializer.create_item((0, 0), table.interned_symbols.search_terminal("IDENTIFIER").unwrap().id),
        initializer.create_item((0, 0), table.interned_symbols.search_terminal("=").unwrap().id),
        initializer.create_item((0, 0), table.interned_symbols.search_terminal("IDENTIFIER").unwrap().id),
        initializer.create_item((0, 0), table.interned_symbols.search_terminal("EOF").unwrap().id),
    ];
    let iterator = ItemVecIterator::new(&items);

    let mut automaton = ParserAutomaton::new(
        &table,
        Bitset::<Vec<u64>>::new(table.interned_symbols.terminals.len()),
        initializer,
        iterator,
        Vec::new(),
        shred_core::table::StateId(0),
    );
    let result = automaton.run().unwrap();
    //println!("{:?}", automaton.item_initializer.items[result.0]);
    //println!("{:?}", automaton.item_initializer.items[5]);
    //println!("{:?}", automaton.item_initializer.items[0]);
    //println!("{:?}", automaton.item_initializer.items[1]);
    //println!("{:?}", automaton.item_initializer.items[2]);
}
