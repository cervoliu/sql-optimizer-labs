#![allow(unused)]

use std::hash::Hash;

use egg::{define_language, Analysis, DidMerge, Id};

pub mod agg;
pub mod expr;
pub mod plan;
mod value;

pub use value::*;

pub type RecExpr = egg::RecExpr<Expr>;
pub type EGraph = egg::EGraph<Expr, ExprAnalysis>;
pub type Rewrite = egg::Rewrite<Expr, ExprAnalysis>;

define_language! {
    pub enum Expr {
        // values
        Constant(Value),            // null, true, 1, 'hello'
        Column(Column),             // t.a, b, c

        // list
        "list" = List(Vec<Id>),

        // operations
        "isnull" = IsNull(Id),
        "-" = Neg(Id),
        "+" = Add([Id; 2]),
        "-" = Sub([Id; 2]),
        "*" = Mul([Id; 2]),
        "/" = Div([Id; 2]),
        "=" = Eq([Id; 2]),
        "<>" = Neq([Id; 2]),
        ">" = Gt([Id; 2]),
        "<" = Lt([Id; 2]),
        ">=" = Gte([Id; 2]),
        "<=" = Lte([Id; 2]),
        "not" = Not(Id),
        "and" = And([Id; 2]),
        "or" = Or([Id; 2]),
        "xor" = Xor([Id; 2]),
    }
}

/// The unified analysis for all rules.
#[derive(Default)]
pub struct ExprAnalysis;

/// The analysis data associated with each eclass.
///
/// See [`egg::Analysis`] for how data is being processed.
#[derive(Debug)]
pub struct Data {
    // TODO: add analysis data
}

impl Analysis<Expr> for ExprAnalysis {
    type Data = Data;

    /// Analyze a node and give the result.
    fn make(egraph: &EGraph, enode: &Expr) -> Self::Data {
        todo!()
    }

    /// Merge the analysis data with previous one.
    fn merge(&mut self, to: &mut Self::Data, from: Self::Data) -> DidMerge {
        todo!()
    }

    /// Modify the graph after analyzing a node.
    fn modify(egraph: &mut EGraph, id: Id) {
        todo!()
    }
}
