use egg::Language;

use super::*;

/// The data type of aggregation analysis.
pub type AggSet = Vec<Expr>;

/// Returns all aggregations in the tree.
///
/// If there is an agg over agg, only the outer aggregate is returned here.
pub fn analyze_aggs(egraph: &EGraph, enode: &Expr) -> AggSet {
    use Expr::*;
    let x = |i: &Id| egraph[*i].data.aggs.clone();
    match enode {
        Max(_) | Min(_) | Sum(_) | Avg(_) | Count(_) => vec![enode.clone()],
        Nested(_) | List(_) | Neg(_) | Not(_) | IsNull(_) | Add(_) | Sub(_) | Mul(_) | Div(_)
        | Eq(_) | Neq(_) | Gt(_) | Lt(_) | Gte(_) | Lte(_) | And(_) | Or(_) | Xor(_)
        | Asc(_) | Desc(_) => enode.children().iter().flat_map(x).collect(),
        _ => vec![],
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    // #[error("aggregate function calls cannot be nested")]
    NestedAgg(String),
    // #[error("WHERE clause cannot contain aggregates")]
    AggInWhere,
    // #[error("GROUP BY clause cannot contain aggregates")]
    AggInGroupBy,
    // #[error("column {0} must appear in the GROUP BY clause or be used in an aggregate function")]
    ColumnNotInAgg(String),
}

/// Converts the SELECT statement into a plan tree.
///
/// The nodes of all clauses have been added to the `egraph`.
/// `from`, `where_`... are the ids of their root node.
pub fn plan_select(
    egraph: &mut EGraph,
    from: Id,
    where_: Id,
    having: Id,
    groupby: Id,
    orderby: Id,
    projection: Id,
) -> Result<Id, Error> {
    AggExtractor { egraph }.plan_select(from, where_, having, groupby, orderby, projection)
}

struct AggExtractor<'a> {
    egraph: &'a mut EGraph,
}

impl AggExtractor<'_> {
    fn aggs(&self, id: Id) -> &[Expr] {
        &self.egraph[id].data.aggs
    }

    fn node(&self, id: Id) -> &Expr {
        &self.egraph[id].nodes[0]
    }

    fn list(&self, id: Id) -> &[Id] {
        if let Some(list) = self.egraph[id].iter().find_map(|node| match node {
            Expr::List(list) => Some(list.as_slice()),
            _ => None,
        }) {
            list
        } else if self.egraph[id]
            .iter()
            .any(|node| matches!(node, Expr::Column(col) if col.as_str() == "list"))
        {
            &[]
        } else {
            panic!("expected a list")
        }
    }

    fn plan_select(
        &mut self,
        from: Id,
        where_: Id,
        having: Id,
        groupby: Id,
        orderby: Id,
        projection: Id,
    ) -> Result<Id, Error> {
        if !self.aggs(where_).is_empty() {
            return Err(Error::AggInWhere);
        }
        if !self.aggs(groupby).is_empty() {
            return Err(Error::AggInGroupBy);
        }

        let mut plan = self.egraph.add(Expr::Filter([where_, from]));
        let mut exprs = [projection, having, orderby];
        plan = self.plan_agg(&mut exprs, groupby, plan)?;
        let [projection, having, orderby] = exprs;

        plan = self.egraph.add(Expr::Filter([having, plan]));
        plan = self.egraph.add(Expr::Order([orderby, plan]));
        plan = self.egraph.add(Expr::Proj([projection, plan]));
        Ok(plan)
    }

    /// Extract all aggregates from `exprs` and build an `agg` plan if needed.
    fn plan_agg(&mut self, exprs: &mut [Id], groupby: Id, plan: Id) -> Result<Id, Error> {
        let expr_list = self.egraph.add(Expr::List(exprs.to_vec()));
        let aggs = self.aggs(expr_list).to_vec();
        if aggs.is_empty() && self.list(groupby).is_empty() {
            return Ok(plan);
        }

        for agg in &aggs {
            if agg.children().iter().any(|child| !self.aggs(*child).is_empty()) {
                return Err(Error::NestedAgg(agg.to_string()));
            }
        }

        let mut list: Vec<_> = aggs.into_iter().map(|agg| self.egraph.add(agg)).collect();
        list.sort();
        list.dedup();

        let mut schema = list.clone();
        schema.extend_from_slice(self.list(groupby));

        let aggs = self.egraph.add(Expr::List(list));
        let plan = self.egraph.add(Expr::Agg([aggs, groupby, plan]));

        for id in exprs {
            *id = self.rewrite_agg_in_expr(*id, &schema)?;
        }

        Ok(plan)
    }

    /// Rewrite aggregate outputs and group keys as nested schema references.
    fn rewrite_agg_in_expr(&mut self, id: Id, schema: &[Id]) -> Result<Id, Error> {
        let mut expr = self.node(id).clone();
        if schema.contains(&id) {
            return Ok(self.egraph.add(Expr::Nested(id)));
        }
        if let Expr::Column(col) = &expr {
            return Err(Error::ColumnNotInAgg(col.to_string()));
        }
        for child in expr.children_mut() {
            *child = self.rewrite_agg_in_expr(*child, schema)?;
        }
        Ok(self.egraph.add(expr))
    }
}
