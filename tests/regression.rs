use sql_optimizer_labs::{Column, EGraph, RecExpr};

#[test]
fn scan_columns_do_not_include_table_name() {
    let expr: RecExpr = "(scan t (list a b))".parse().unwrap();
    let mut egraph = EGraph::default();
    let id = egraph.add_expr(&expr);
    egraph.rebuild();

    let cols = &egraph[id].data.columns;
    assert!(cols.contains(&Column::from("a")));
    assert!(cols.contains(&Column::from("b")));
    assert!(!cols.contains(&Column::from("t")));
}
