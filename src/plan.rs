//! Plan optimization rules.

use std::{collections::HashSet, vec};

use super::*;
use egg::{rewrite as rw, Subst, Var};

/// Returns the rules that always improve the plan.
pub fn rules() -> Vec<Rewrite> {
    let mut rules = vec![];
    rules.extend(projection_pushdown_rules());
    rules.extend(join_rules());
    rules.extend(cancel_rules());
    rules.extend(predicate_pushdown_rules());
    rules.extend(merge_rules());
    return rules
}

#[rustfmt::skip]
pub fn cancel_rules() -> Vec<Rewrite> { vec![
    rw!("limit-0";   "(limit 0 0 ?a)" => "(empty ?a)"),
    rw!("limit-null"; "(limit null 0 ?a)" => "?a"),
    rw!("order-null"; "(order (list) ?a)" => "?a"),
    rw!("filter-true";  "(filter true ?a)" => "?a"),
    rw!("filter-false"; "(filter false ?a)" => "(empty ?a)"),
    rw!("inner-join-false"; 
        "(join inner false ?a ?b)" => 
        "(empty (join inner false ?a ?b))"
    ),
    rw!("proj-on-empty"; "(proj ?cols (empty ?a))" => "(empty ?cols)"),
    rw!("filter-on-empty"; "(filter ?cond (empty ?a))" => "(empty ?a)"),
    rw!("order-on-empty"; "(order ?ord (empty ?a))" => "(empty ?a)"),
    rw!("limit-on-empty"; "(limit ?lim ?off (empty ?a))" => "(empty ?a)"),
]}

#[rustfmt::skip]
pub fn merge_rules() -> Vec<Rewrite> { vec![
    rw!("limit-order-topn";
        "(limit ?lim ?off (order ?ord ?a))" => 
        "(topn ?lim ?off ?ord ?a)"
    ),
    rw!("filter-merge";
        "(filter ?cond1 (filter ?cond2 ?a))" => 
        "(filter (and ?cond1 ?cond2) ?a)"
    ),
    rw!("proj-merge";
        "(proj ?cols1 (proj ?cols2 ?a))" => 
        "(proj ?cols1 ?a)"
    ),
]}

#[rustfmt::skip]
pub fn join_rules() -> Vec<Rewrite> { vec![
    rw!("inner-join-assoc";
        "(join inner ?cond2 (join inner ?cond1 ?left ?mid) ?right)" =>
        "(join inner ?cond1 ?left (join inner ?cond2 ?mid ?right))"
        if columns_is_disjoint("?cond2", "?left")
    ),
]}

/// Pushdown projections and prune unused columns.
#[rustfmt::skip]
pub fn projection_pushdown_rules() -> Vec<Rewrite> { vec![
    // TODO: add rules
]}

pub fn predicate_pushdown_rules() -> Vec<Rewrite> { vec![
    rw!("pushdown-filter-join";
        "(filter ?cond (join ?type ?on ?left ?right))" =>
        "(join ?type (and ?on ?cond) ?left ?right)"
    ),
    rw!("pushdown-filter-join-left";
        "(join ?type ?cond ?left ?right)" =>
        "(join ?type true (filter ?cond ?left) ?right)"
        if columns_is_subset("?cond", "?left")
    ),
    rw!("pushdown-filter-join-left-1";
        "(join ?type (and ?cond1 ?cond2) ?left ?right)" =>
        "(join ?type ?cond2 (filter ?cond1 ?left) ?right)"
        if columns_is_subset("?cond1", "?left")
    ),
    rw!("pushdown-filter-join-right";
        "(join ?type ?cond ?left ?right)" =>
        "(join ?type true ?left (filter ?cond ?right))"
        if columns_is_subset("?cond", "?right")
    ),
    rw!("pushdown-filter-join-right-2";
        "(join ?type (and ?cond1 ?cond2) ?left ?right)" =>
        "(join ?type ?cond1 ?left (filter ?cond2 ?right))"
        if columns_is_subset("?cond2", "?right")
    )
]}
pub type ColumnSet = HashSet<Column>;

pub fn analyze_columns(egraph: &EGraph, enode: &Expr) -> ColumnSet {
    use Expr::*;
    let x = |i: &Id| &egraph[*i].data.columns;
    match enode {
        Column(col) => [*col].into_iter().collect(),
        Proj([exprs, _]) => x(exprs).clone(),
        Agg([exprs, group_keys, _]) => {
            x(exprs).union(x(group_keys)).cloned().collect()
        },
        _ => (enode.children().iter())
            .flat_map(|id| x(id).iter().cloned())
            .collect()
    }
}

/// Not to be confused with the above analyze_columns,
/// where we take the union of columns from children.
/// Here, we are merging the eclass data of two equivalent enodes
/// we simply keep the smaller one
pub fn merge(to: &mut ColumnSet, from: ColumnSet) -> DidMerge {
    if from.len() < to.len() {
        *to = from;
        DidMerge(true, false)
    } else {
        DidMerge(false, true)
    }
}

fn columns_is_subset(cond: &str, table: &str) -> impl Fn(&mut EGraph, Id, &Subst) -> bool {
    columns_is(cond, table, ColumnSet::is_subset)
}

fn columns_is_disjoint(cond: &str, table: &str) -> impl Fn(&mut EGraph, Id, &Subst) -> bool {
    columns_is(cond, table, ColumnSet::is_disjoint)
}

fn columns_is(
    cond: &str,
    table: &str,
    f: impl Fn(&ColumnSet, &ColumnSet) -> bool + 'static
) -> impl Fn(&mut EGraph, Id, &Subst) -> bool {
    let cond_var = cond.parse::<Var>().unwrap();
    let table_var = table.parse::<Var>().unwrap();
    move |egraph, _id, subst| {
        let cond_cols = &egraph[subst[cond_var]].data.columns;
        let table_cols = &egraph[subst[table_var]].data.columns;
        f(cond_cols, table_cols)
    }
}