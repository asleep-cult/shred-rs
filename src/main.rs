mod ast;
mod scanner;
mod parser;
mod symbols;
mod lowering;
mod table;
mod generator;
mod errors;

fn main() {
    let source = std::fs::read_to_string("./grammar_test").unwrap();
    let scanner = scanner::Scanner::new(&source);
    let parser = parser::GrammarParser::new(scanner);
    let arena = parser.parse_ast().unwrap();

    let ctx = lowering::LoweringContext::new();
    let interned_symbols = ctx.lower_symbols(arena).unwrap();

    //for nonterminal in &interned_symbols.nonterminals {
    //    let mut buffer = String::new();
    //    nonterminal.format_string(&interned_symbols, &mut buffer).unwrap();
    //    print!("{}", buffer);
    //}

    let mut ctx = generator::GeneratorContext::new(&interned_symbols);
    let table = ctx.compute_table();
    //let mut fp = std::fs::File::create("./table.out").unwrap();
    //table.dump_table(&mut fp).unwrap();
}
