//! This file defines the AST for the grammar.

pub struct NodeId(pub u32);

pub struct NodeRange {
    start: u32,
    end: u32,
}

pub enum RuleKind {
    Star(NodeId),
    Plus(NodeId),
    Optional(NodeId),
    Alternative { left: NodeId, right: NodeId },
    Group { items: NodeRange },
    Name(String),
}
