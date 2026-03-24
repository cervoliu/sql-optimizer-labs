//! Plan optimization rules.

use std::{collections::HashSet, vec};

use super::*;
use egg::{Applier, Pattern, PatternAst, Subst, Symbol, Var, rewrite as rw};

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
    rw!("hash-join-on-one-eq";
        "(join ?type (= ?el ?er) ?left ?right)" =>
        "(hashjoin ?type (list ?el) (list ?er) ?left ?right)"
        if columns_is_subset("?el", "?left")
        if columns_is_subset("?er", "?right")
    ),
    rw!("hash-join-on-two-eq";
        "(join ?type (and (= ?l1 ?r1) (= ?l2 ?r2)) ?left ?right)" =>
        "(hashjoin ?type (list ?l1 ?l2) (list ?r1 ?r2) ?left ?right)"
        if columns_is_subset("?l1", "?left")
        if columns_is_subset("?l2", "?left")
        if columns_is_subset("?r1", "?right")
        if columns_is_subset("?r2", "?right")
    ),
]}

/// Pushdown projections and prune unused columns.
#[rustfmt::skip]
pub fn projection_pushdown_rules() -> Vec<Rewrite> { vec![
    rw!("pushdown-proj-order";
        "(proj ?exprs (order ?keys ?child))" =>
        "(proj ?exprs (order ?keys (proj (column-merge ?exprs ?keys) ?child)))"
    ),
    rw!("pushdown-proj-topn";
        "(proj ?exprs (topn ?limit ?offset ?keys ?child))" =>
        "(proj ?exprs (topn ?limit ?offset ?keys (proj (column-merge ?exprs ?keys) ?child)))"
    ),
    rw!("pushdown-proj-filter";
        "(proj ?exprs (filter ?cond ?child))" =>
        "(proj ?exprs (filter ?cond (proj (column-merge ?exprs ?cond) ?child)))"
    ),
    rw!("pushdown-proj-agg";
        "(proj ?exprs (agg ?aggs ?group_keys ?child))" =>
        "(proj ?exprs (agg ?aggs ?group_keys (proj (column-merge ?aggs ?group_keys) ?child)))"
    ),
    rw!("pushdown-proj-join";
        "(proj ?exprs (join ?type ?on ?left ?right))" =>
        "(proj ?exprs (join ?type ?on
            (proj (column-prune ?left (column-merge ?exprs ?on)) ?left)
            (proj (column-prune ?right (column-merge ?exprs ?on)) ?right)
        ))"
    ),
    rw!("pushdown-proj-scan";
        "(proj ?exprs (scan ?table ?columns))" =>
        "(scan ?table (column-prune ?exprs ?columns))"
    ),
    rw!("column-merge";
        "(column-merge ?list1 ?list2)" =>
        { ColumnMerge {
            lists: [var("?list1"), var("?list2")],
        }}
    ),
    rw!("column-prune";
        "(column-prune ?filter ?list)" =>
        { ColumnPrune {
            filter: var("?filter"),
            list: var("?list"),
        }}
        if is_list("?list")
    ),
]}

struct ColumnMerge {
    lists: [Var; 2],
}

impl Applier<Expr, ExprAnalysis> for ColumnMerge {
    fn apply_one(
        &self,
        egraph: &mut EGraph,
        eclass: Id,
        subst: &Subst,
        _searcher_ast: Option<&PatternAst<Expr>>,
        _rule_name: Symbol,
    ) -> Vec<Id> {
        let list1 = &egraph[subst[self.lists[0]]].data.columns;
        let list2 = &egraph[subst[self.lists[1]]].data.columns;
        let mut list: Vec<&Column> = list1.union(list2).collect();
        list.sort_unstable_by_key(|col| col.as_str());
        let list = list
            .into_iter()
            .map(|col| egraph.lookup(Expr::Column(col.clone())).unwrap())
            .collect();
        let id = egraph.add(Expr::List(list));

        if egraph.union(eclass, id) {
            vec![eclass]
        } else {
            vec![]
        }
    }
}

struct ColumnPrune {
    filter: Var,
    list: Var,
}

impl Applier<Expr, ExprAnalysis> for ColumnPrune {
    fn apply_one(
        &self,
        egraph: &mut EGraph,
        eclass: Id,
        subst: &Subst,
        _searcher_ast: Option<&PatternAst<Expr>>,
        _rule_name: Symbol,
    ) -> Vec<Id> {
        let columns = &egraph[subst[self.filter]].data.columns;
        let list = egraph[subst[self.list]].as_list();
        let pruned = list
            .iter()
            .cloned()
            .filter(|id| egraph[*id].data.columns.is_subset(columns))
            .collect();
        let id = egraph.add(Expr::List(pruned));

        if egraph.union(eclass, id) {
            vec![eclass]
        } else {
            vec![]
        }
    }
}

fn is_list(v: &str) -> impl Fn(&mut EGraph, Id, &Subst) -> bool {
    let v = var(v);
    move |egraph, _, subst| {
        egraph[subst[v]]
            .iter()
            .any(|node| matches!(node, Expr::List(_)))
    }
}


pub fn predicate_pushdown_rules() -> Vec<Rewrite> { vec![
    pushdown("filter", "?cond", "order", "?keys"),
    pushdown("filter", "?cond", "limit", "?limit ?offset"),
    pushdown("filter", "?cond", "topn", "?limit ?offset ?keys"),
    rw!("pushdown-filter-over-inner-join";
        "(filter ?cond (join inner ?on ?left ?right))" =>
        "(join inner (and ?on ?cond) ?left ?right)"
    ),
    rw!("pushdown-filter-over-inner-join-left";
        "(join inner ?cond ?left ?right)" =>
        "(join inner true (filter ?cond ?left) ?right)"
        if columns_is_subset("?cond", "?left")
    ),
    rw!("pushdown-filter-over-inner-join-left-1";
        "(join inner (and ?cond1 ?cond2) ?left ?right)" =>
        "(join inner ?cond2 (filter ?cond1 ?left) ?right)"
        if columns_is_subset("?cond1", "?left")
    ),
    rw!("pushdown-filter-over-inner-join-right";
        "(join inner ?cond ?left ?right)" =>
        "(join inner true ?left (filter ?cond ?right))"
        if columns_is_subset("?cond", "?right")
    ),
    rw!("pushdown-filter-over-inner-join-right-2";
        "(join inner (and ?cond1 ?cond2) ?left ?right)" =>
        "(join inner ?cond1 ?left (filter ?cond2 ?right))"
        if columns_is_subset("?cond2", "?right")
    )
]}

/// Returns a rule to pushdown plan `a` through `b`.
fn pushdown(a: &str, a_args: &str, b: &str, b_args: &str) -> Rewrite {
    let name = format!("pushdown-{a}-over-{b}");
    let searcher = format!("({a} {a_args} ({b} {b_args} ?child))")
        .parse::<Pattern<_>>()
        .unwrap();
    let applier = format!("({b} {b_args} ({a} {a_args} ?child))")
        .parse::<Pattern<_>>()
        .unwrap();
    Rewrite::new(name, searcher, applier).unwrap()
}

pub type ColumnSet = HashSet<Column>;

pub fn analyze_columns(egraph: &EGraph, enode: &Expr) -> ColumnSet {
    use Expr::*;
    let x = |i: &Id| &egraph[*i].data.columns;
    match enode {
        Column(col) => [*col].into_iter().collect(),
        Scan([_, cols]) => x(cols).clone(),
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
