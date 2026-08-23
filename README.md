# shred-rs
Shred means shift-reduce.
For my project [Typethon](https://github.com/asleep-cult/typethon/tree/master/typethon/grammar), I implemented
a LR(1) parser with a pushdown automaton in pure Python.
The goal of this project is to port the automaton and parser generator to Rust, with
PyO3 bindings, allowing the generator and automaton to be called from Python.
I have no idea how to use Rust.

The grammar syntax should look like this:
```
PLUS = "+"

expr:
    | expr "+" term => create_binary_op
```

There are a few important questions that need to be answered:
* How will the grammar define terminal symbols?
  - It should probably be defined at the top of the grammar explicitly
  - The automaton should accept a scan function pointer that creates
            whatever kind of token it wants to.
  - The generated parser should maintain a list of terminal symbols
            so the scanner can agree on the id to provide. 
* How will the type in the scanner stack be represented?

Notes on progress:
- [X] `ast.py` -> `ast.rs`: Definitions for meta-grammar AST
- [ ] `automaton.py`: Pushdown automaton for parsing with generated parse table
- [ ] `frozen.py`: Serializable versions of the symbol/parse table which the automaton uses
- [ ] `generator.py`: Convert the AST into a high-level representation and generate the parse table
- [X] `generator.py` -> `lowering.rs`: Lower AST into a graph of symbols.
- [X] `parser.py` -> `parser.rs`: Parser for the meta-grammar
- [X] `symbols.py` -> `symbols.rs`: High-level representation of the AST
- [X] `tokens.py` -> `scanner.rs`: Token definitions for the meta-grammar, Rust version
    has a scanner because the Python implementation used the Typethon scanner. 
