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

    let mut ctx = generator::GeneratorContext::new(&interned_symbols);
    let table = ctx.compute_table();
    table.dump_table(&mut std::io::stdout()).unwrap();
}
