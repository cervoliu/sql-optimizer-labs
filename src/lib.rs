#![allow(unused)]

use std::hash::Hash;

use egg::{define_language, Analysis, DidMerge, Id};

pub mod agg;
pub mod expr;
pub mod plan;
mod value;

pub use value::*;

use crate::expr::{ConstValue, eval_constant};

pub type RecExpr = egg::RecExpr<Expr>;
pub type EGraph = egg::EGraph<Expr, ExprAnalysis>;
pub type Rewrite = egg::Rewrite<Expr, ExprAnalysis>;

define_language! {
    pub enum Expr {
        // values
        Constant(Value),            // null, true, 1, 'hello'
        Column(Column),             // t.a, b, c
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

        // Aggregations
        "max" = Max(Id),
        "min" = Min(Id),
        "sum" = Sum(Id),
        "avg" = Avg(Id),
        "count" = Count(Id),

        // plans
        // we need an empty node that produces zero rows
        // while preserving the schema
        "empty" = Empty(Id),
        "scan" = Scan([Id; 2]),
        "values" = Values(Id),
        "proj" = Project([Id; 2]),
        "filter" = Filter([Id; 2]),
        "order" = Order([Id; 2]),
        "asc" = Asc(Id), "desc" = Desc(Id),
        "limit" = Limit([Id; 3]),
        "topn" = TopN([Id; 4]),
        "agg" = Aggregate([Id; 3]),
        "join" = Join([Id; 4]),
        "hashjoin" = HashJoin([Id; 5]),
        "inner" = Inner, "left_outer" = LeftOuter,
        "right_outer" = RightOuter, "full_outer" = FullOuter,
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
    pub constant: ConstValue,
}

impl Analysis<Expr> for ExprAnalysis {
    type Data = Data;

    /// Analyze a node and give the result.
    fn make(egraph: &EGraph, enode: &Expr) -> Self::Data {
        Data {
            constant: eval_constant(egraph, enode)
        }
    }

    /// Merge the analysis data with previous one.
    fn merge(&mut self, to: &mut Self::Data, from: Self::Data) -> DidMerge {
        egg::merge_max(&mut to.constant, from.constant)
    }

    /// Modify the graph after analyzing a node.
    fn modify(egraph: &mut EGraph, id: Id) {
        if let Some(val) = &egraph[id].data.constant {
            let new_id = egraph.add(Expr::Constant(val.clone()));
            egraph.union(id, new_id);
        }
    }
}
