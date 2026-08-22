use crate::scanner::Scanner;

pub struct GrammarParser<'a> {
    scanner: Scanner<'a>,
}

impl<'t> GrammarParser<'t> {
    fn new(scanner: Scanner<'t>) -> Self {
        return Self { scanner }
    }
}
