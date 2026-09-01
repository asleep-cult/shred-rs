use std::time::Instant;

use shred_generator::generator::TableGenerator;
use shred_core::bitset::Bitset;
use shred_automaton::automaton::{DefaultInitializer, DefaultItem, ItemInitializer, ItemVecIterator, ParserAutomaton};

fn main() {
    let source = std::fs::read_to_string("./grammar_test").unwrap();
    let table = TableGenerator::generate_from_grammar(&source);

    let mut initializer = DefaultInitializer::new();
    let names = vec![
        "type",
        "IDENTIFIER",
        "=",
        "IDENTIFIER",
        "of",
        "'",
        "IDENTIFIER",
        "|",
        "IDENTIFIER",
        "of",
        "{",
        "IDENTIFIER",
        ":",
        "'",
        "IDENTIFIER",
        "}",
        "NEWLINE",
        "type",
        "IDENTIFIER",
        "=",
        "IDENTIFIER",
        "|",
        "IDENTIFIER",
        "|",
        "IDENTIFIER",
        "NEWLINE",
        "type",
        "IDENTIFIER",
        "=",
        "{",
        "IDENTIFIER",
        ":",
        "{",
        "IDENTIFIER",
        ":",
        "IDENTIFIER",
        "}",
        ",",
        "IDENTIFIER",
        ":",
        "(",
        "IDENTIFIER",
        ",",
        ")",
        "}",
        "NEWLINE",
        "EOF",
    ];
    let items: Vec<_> = names.iter()
        .map(|name| {
            initializer.create_item((0, 0),
                table.interned_symbols.search_terminal(name)
                .unwrap_or_else(|| table.interned_symbols.search_nonterminal(&name).unwrap()).id).unwrap()
        })
        .collect::<Vec<DefaultItem>>()
        .into_iter()
        .cycle()
        .take(164)
        .collect();

    let iterator = ItemVecIterator::new(&items);
    let mut automaton = ParserAutomaton::new(
        &table,
        Bitset::<Vec<u64>>::new(table.interned_symbols.terminals.len()),
        initializer,
        iterator,
        Vec::new(),
        shred_core::table::StateId(0),
    ).unwrap();
    let start = Instant::now();
    let result = automaton.run();
    match result {
        Err(shred_automaton::automaton::AutomatonErrorKind::UnexpectedToken { item, state_id }) => {
            println!("{:?}", automaton.item_initializer.items[item.0]);
        }
        _ => {}
    }
    println!("That took {} ms", start.elapsed().as_millis());

}
