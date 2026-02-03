//! Plan optimization rules.

use std::{collections::HashSet, vec};

use super::*;
use egg::rewrite as rw;

/// Returns the rules that always improve the plan.
pub fn rules() -> Vec<Rewrite> {
    let mut rules = vec![];
    rules.extend(projection_pushdown_rules());
    rules.extend(join_rules());
    rules.extend(cancel_rules());
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
    // TODO: add rules
]}

/// Pushdown projections and prune unused columns.
#[rustfmt::skip]
pub fn projection_pushdown_rules() -> Vec<Rewrite> { vec![
    // TODO: add rules
]}
