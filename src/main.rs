mod ast;
mod scanner;
mod parser;
mod symbols;
mod lowering;
mod table;
mod generator;
mod errors;
mod bitset;
mod lr;

fn main() {
    let source = std::fs::read_to_string("./grammar_test").unwrap();
    let scanner = scanner::Scanner::new(&source);
    let parser = parser::GrammarParser::new(scanner);
    let arena = parser.parse_ast().unwrap();

    let ctx = lowering::LoweringContext::new(arena);
    let interned_symbols = ctx.lower_symbols().unwrap();

    //for nonterminal in &interned_symbols.nonterminals {
    //    let mut buffer = String::new();
    //    nonterminal.format_string(&interned_symbols, &mut buffer).unwrap();
    //    print!("{}", buffer);
    //}

    let mut ctx = generator::ComputationEngine::new(&interned_symbols);
    let table = ctx.compute_table();
    //let mut fp = std::fs::File::create("./table.out").unwrap();
    println!("done");
    //table.dump_table(&mut fp).unwrap();
}
