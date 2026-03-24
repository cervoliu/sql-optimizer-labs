#![allow(unused)]

use std::hash::Hash;

use egg::{Analysis, DidMerge, Id, Language, Var, define_language};

pub mod agg;
pub mod expr;
pub mod plan;
mod value;

pub use value::*;

use crate::expr::{ConstValue, eval_constant};
use crate::plan::{ColumnSet, analyze_columns, merge};

pub type RecExpr = egg::RecExpr<Expr>;
pub type EGraph = egg::EGraph<Expr, ExprAnalysis>;
pub type Rewrite = egg::Rewrite<Expr, ExprAnalysis>;

define_language! {
    pub enum Expr {
        // values
        Constant(Value),            // null, true, 1, 'hello'
        Column(Column),             // t.a, b, c
        "`" = Nested(Id),           // (` expr) wrapper to prevent rewrites from breaking agg schema mapping
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
        // while preserving the schema (child node)
        "empty" = Empty(Id),
        "scan" = Scan([Id; 2]),     // (scan table [column..])
        "values" = Values(Id),      // (values [row[column..]..])
        "proj" = Proj([Id; 2]),  // (proj [column..] child)
        "filter" = Filter([Id; 2]), // (filter condition child)
        "order" = Order([Id; 2]),   // (order [order_key..] child)
            "asc" = Asc(Id),            // order key types
            "desc" = Desc(Id),
        "limit" = Limit([Id; 3]),   // (limit limit offset child)
        "topn" = TopN([Id; 4]),     // (topn limit offset [order_key..] child)
        "agg" = Agg([Id; 3]), // (agg aggs=[expr..] group_keys=[expr..] child)
                                        // expressions must be aggs
                                        // output = aggs || group_keys
        "join" = Join([Id; 4]),     // (join type condition left right)
        "hashjoin" = HashJoin([Id; 5]), // (hashjoin type [left_key..] [right_key..] left right)
                                        // left and right keys must match
            "inner" = Inner,            // join types
            "left_outer" = LeftOuter,
            "right_outer" = RightOuter,
            "full_outer" = FullOuter,
        // helper internal nodes used in projection pushdown rules
        "column-merge" = ColumnMerge([Id; 2]), // (column-merge list1 list2)
                                                    // return a list of columns from list1 and list2
        "column-prune" = ColumnPrune([Id; 2]), // (column-prune filter list)
                                                    // remove element from `list` whose column set is not a subset of `filter`
    }
}

impl Expr {
    fn as_list(&self) -> &[Id] {
        match self {
            Expr::List(list) => list,
            _ => panic!("expected a list"),
        }
    }
}

trait ExprExt {
    fn as_list(&self) -> &[Id];
}

impl<D> ExprExt for egg::EClass<Expr, D> {
    fn as_list(&self) -> &[Id] {
        self.iter()
            .find_map(|e| match e {
                Expr::List(list) => Some(list),
                _ => None,
            })
            .expect("not list")
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
    pub columns: ColumnSet,
    pub aggs: agg::AggSet,
}

impl Analysis<Expr> for ExprAnalysis {
    type Data = Data;

    /// Analyze a node and give the result.
    fn make(egraph: &EGraph, enode: &Expr) -> Self::Data {
        Data {
            constant: eval_constant(egraph, enode),
            columns: analyze_columns(egraph, enode),
            aggs: agg::analyze_aggs(egraph, enode),
        }
    }

    /// Merge the analysis data with previous one.
    fn merge(&mut self, to: &mut Self::Data, from: Self::Data) -> DidMerge {
        let merge_constant = egg::merge_max(&mut to.constant, from.constant);
        let merge_columns = plan::merge(&mut to.columns, from.columns);
        let merge_aggs = egg::merge_max(&mut to.aggs, from.aggs);
        merge_constant | merge_columns | merge_aggs
    }

    /// Modify the graph after analyzing a node.
    fn modify(egraph: &mut EGraph, id: Id) {
        if let Some(val) = &egraph[id].data.constant {
            let new_id = egraph.add(Expr::Constant(val.clone()));
            egraph.union(id, new_id);
        }
    }
}

/// Create a [`Var`] from string.
///
/// This is a helper function for submodules.
fn var(s: &str) -> Var {
    s.parse().expect("invalid variable")
}
