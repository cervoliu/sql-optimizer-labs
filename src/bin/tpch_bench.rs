use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use egg::{AstSize, Extractor, Id, Language, Runner};
use sql_optimizer_labs::{
    EGraph, Expr, ExprAnalysis, RecExpr,
    agg::{self, Error},
    expr, plan,
};

struct QuerySpec {
    name: String,
    family: String,
    note: String,
    sql: String,
    select: String,
    from: String,
    where_: String,
    having: String,
    groupby: String,
    orderby: String,
    limit: String,
    offset: String,
}

#[derive(Default, Clone)]
struct PlanStats {
    total_nodes: usize,
    plan_nodes: usize,
    scan_nodes: usize,
    scan_columns: usize,
    filter_nodes: usize,
    proj_nodes: usize,
    order_nodes: usize,
    topn_nodes: usize,
    agg_nodes: usize,
    join_nodes: usize,
    hashjoin_nodes: usize,
    empty_nodes: usize,
    max_depth: usize,
    empty_scan_schemas: usize,
    bare_list_symbols: usize,
    hashjoin_empty_keys: usize,
    hashjoin_non_column_keys: usize,
}

struct QueryResult {
    name: String,
    family: String,
    note: String,
    sql: String,
    build_ms: f64,
    optimize_ms_median: f64,
    optimize_ms_min: f64,
    optimize_ms_max: f64,
    iterations: usize,
    egraph_nodes: usize,
    egraph_classes: usize,
    memo_size: usize,
    rule_applications: usize,
    best_cost: usize,
    stop_reason: String,
    before: PlanStats,
    after: PlanStats,
    before_plan: String,
    after_plan: String,
    rule_counts: Vec<(String, usize)>,
}

fn main() {
    let args = Args::parse(env::args().skip(1).collect());
    let queries = workload();
    let rules = all_rules();
    let mut results = Vec::with_capacity(queries.len());

    for spec in &queries {
        match benchmark_query(spec, args.samples, &rules) {
            Ok(result) => results.push(result),
            Err(err) => {
                eprintln!("failed to benchmark {}: {:?}", spec.name, err);
                std::process::exit(1);
            }
        }
    }

    let csv = render_csv(&results);
    let markdown = render_markdown(&results, args.samples);

    if let Some(path) = args.csv {
        write_output(path, &csv);
    }
    if let Some(path) = args.markdown {
        write_output(path, &markdown);
    }

    print!("{markdown}");
}

#[derive(Default)]
struct Args {
    samples: usize,
    csv: Option<PathBuf>,
    markdown: Option<PathBuf>,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut parsed = Self {
            samples: 5,
            csv: None,
            markdown: None,
        };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--samples" => {
                    i += 1;
                    parsed.samples = args
                        .get(i)
                        .expect("missing value after --samples")
                        .parse()
                        .expect("invalid integer for --samples");
                }
                "--csv" => {
                    i += 1;
                    parsed.csv = Some(PathBuf::from(
                        args.get(i).expect("missing value after --csv"),
                    ));
                }
                "--markdown" => {
                    i += 1;
                    parsed.markdown = Some(PathBuf::from(
                        args.get(i).expect("missing value after --markdown"),
                    ));
                }
                flag => panic!("unknown flag: {flag}"),
            }
            i += 1;
        }
        parsed
    }
}

fn write_output(path: PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create output directory");
    }
    fs::write(path, contents).expect("failed to write benchmark report");
}

fn benchmark_query(
    spec: &QuerySpec,
    samples: usize,
    rules: &[egg::Rewrite<Expr, ExprAnalysis>],
) -> Result<QueryResult, Error> {
    let build_start = Instant::now();
    let initial = build_initial_plan(spec)?;
    let build_ms = build_start.elapsed().as_secs_f64() * 1000.0;
    let before = collect_plan_stats(&initial);
    let before_plan = initial.to_string();

    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        let _runner = Runner::default()
            .with_iter_limit(40)
            .with_node_limit(50_000)
            .with_time_limit(Duration::from_secs(5))
            .with_expr(&initial)
            .run(rules);
        times.push(start.elapsed());
    }

    let runner = Runner::default()
        .with_iter_limit(40)
        .with_node_limit(50_000)
        .with_time_limit(Duration::from_secs(5))
        .with_expr(&initial)
        .run(rules);
    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (best_cost, best) = extractor.find_best(runner.roots[0]);
    let after = collect_plan_stats(&best);
    let after_plan = best.to_string();
    let iterations = runner.iterations.len();
    let rule_applications = runner
        .iterations
        .iter()
        .map(|iteration| iteration.applied.values().sum::<usize>())
        .sum();
    let mut aggregated_rules: BTreeMap<String, usize> = BTreeMap::new();
    for iteration in &runner.iterations {
        for (rule, count) in &iteration.applied {
            *aggregated_rules.entry(rule.to_string()).or_default() += count;
        }
    }
    let mut rule_counts: Vec<_> = aggregated_rules.into_iter().collect();
    rule_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut millis: Vec<f64> = times
        .iter()
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .collect();
    millis.sort_by(f64::total_cmp);
    let optimize_ms_median = median(&millis);
    let optimize_ms_min = *millis.first().unwrap_or(&0.0);
    let optimize_ms_max = *millis.last().unwrap_or(&0.0);

    Ok(QueryResult {
        name: spec.name.clone(),
        family: spec.family.clone(),
        note: spec.note.clone(),
        sql: spec.sql.clone(),
        build_ms,
        optimize_ms_median,
        optimize_ms_min,
        optimize_ms_max,
        iterations,
        egraph_nodes: runner.egraph.total_size(),
        egraph_classes: runner.egraph.number_of_classes(),
        memo_size: runner.egraph.total_size(),
        rule_applications,
        best_cost,
        stop_reason: format!("{:?}", runner.stop_reason),
        before,
        after,
        before_plan,
        after_plan,
        rule_counts,
    })
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn build_initial_plan(spec: &QuerySpec) -> Result<RecExpr, Error> {
    let mut egraph = EGraph::default();
    let projection = egraph.add_expr(&spec.select.parse().unwrap());
    let from = egraph.add_expr(&spec.from.parse().unwrap());
    let where_ = egraph.add_expr(&spec.where_.parse().unwrap());
    let having = egraph.add_expr(&spec.having.parse().unwrap());
    let groupby = egraph.add_expr(&spec.groupby.parse().unwrap());
    let orderby = egraph.add_expr(&spec.orderby.parse().unwrap());
    let mut root = agg::plan_select(
        &mut egraph,
        from,
        where_,
        having,
        groupby,
        orderby,
        projection,
    )?;
    if spec.limit != "null" {
        let limit = egraph.add_expr(&spec.limit.parse().unwrap());
        let offset = egraph.add_expr(&spec.offset.parse().unwrap());
        root = egraph.add(Expr::Limit([limit, offset, root]));
    }
    Ok(recexpr_from_eclass(&egraph, root))
}

fn recexpr_from_eclass(egraph: &EGraph, id: Id) -> RecExpr {
    let get_node = |id| egraph[id].nodes[0].clone();
    get_node(id).build_recexpr(get_node)
}

fn collect_plan_stats(expr: &RecExpr) -> PlanStats {
    let nodes = expr.as_ref();
    let mut stats = PlanStats::default();
    let mut depths = vec![0usize; nodes.len()];

    for (index, node) in nodes.iter().enumerate() {
        let depth = 1 + node
            .children()
            .iter()
            .map(|id| depths[usize::from(*id)])
            .max()
            .unwrap_or(0);
        depths[index] = depth;
        stats.total_nodes += 1;
        stats.max_depth = stats.max_depth.max(depth);

        match node {
            Expr::Scan([_, cols]) => {
                stats.plan_nodes += 1;
                stats.scan_nodes += 1;
                let width = list_len(expr, *cols);
                stats.scan_columns += width;
                if width == 0 {
                    stats.empty_scan_schemas += 1;
                }
            }
            Expr::Values(_) => stats.plan_nodes += 1,
            Expr::Proj(_) => {
                stats.plan_nodes += 1;
                stats.proj_nodes += 1;
            }
            Expr::Filter(_) => {
                stats.plan_nodes += 1;
                stats.filter_nodes += 1;
            }
            Expr::Order(_) => {
                stats.plan_nodes += 1;
                stats.order_nodes += 1;
            }
            Expr::Limit(_) => stats.plan_nodes += 1,
            Expr::TopN(_) => {
                stats.plan_nodes += 1;
                stats.topn_nodes += 1;
            }
            Expr::Agg(_) => {
                stats.plan_nodes += 1;
                stats.agg_nodes += 1;
            }
            Expr::Join(_) => {
                stats.plan_nodes += 1;
                stats.join_nodes += 1;
            }
            Expr::HashJoin(_) => {
                stats.plan_nodes += 1;
                stats.hashjoin_nodes += 1;
                if let Expr::HashJoin([_, left_keys, right_keys, _, _]) = node {
                    stats.hashjoin_empty_keys += usize::from(list_len(expr, *left_keys) == 0);
                    stats.hashjoin_empty_keys += usize::from(list_len(expr, *right_keys) == 0);
                    stats.hashjoin_non_column_keys += non_column_key_count(expr, *left_keys);
                    stats.hashjoin_non_column_keys += non_column_key_count(expr, *right_keys);
                }
            }
            Expr::Empty(_) => {
                stats.plan_nodes += 1;
                stats.empty_nodes += 1;
            }
            Expr::Column(col) if col.as_str() == "list" => {
                stats.bare_list_symbols += 1;
            }
            _ => {}
        }
    }

    stats
}

fn list_len(expr: &RecExpr, id: Id) -> usize {
    match &expr[id] {
        Expr::List(ids) => ids.len(),
        _ => 0,
    }
}

fn non_column_key_count(expr: &RecExpr, id: Id) -> usize {
    match &expr[id] {
        Expr::List(ids) => ids
            .iter()
            .filter(|item| !matches!(&expr[**item], Expr::Column(_)))
            .count(),
        _ => 0,
    }
}

fn all_rules() -> Vec<egg::Rewrite<Expr, ExprAnalysis>> {
    let mut rules = expr::rules();
    rules.extend(plan::rules());
    rules
}

fn workload() -> Vec<QuerySpec> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmarks/tpch");
    let spec_dir = root.join("specs");
    let sql_dir = root.join("sql");
    let mut entries: Vec<_> = fs::read_dir(&spec_dir)
        .expect("failed to read benchmarks/tpch/specs")
        .map(|entry| entry.expect("failed to read spec entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("ir"))
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|path| load_query_spec(path, &sql_dir))
        .collect()
}

fn load_query_spec(path: PathBuf, sql_dir: &PathBuf) -> QuerySpec {
    let text = fs::read_to_string(&path).expect("failed to read query spec");
    let mut meta = HashMap::<String, String>::new();
    let mut sections = HashMap::<String, String>::new();
    let mut current_section: Option<String> = None;
    let mut buffer = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            if current_section.is_some() && !buffer.ends_with('\n') {
                buffer.push('\n');
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(section) = current_section.take() {
                sections.insert(section, buffer.trim().to_string());
                buffer.clear();
            }
            current_section = Some(line[1..line.len() - 1].to_string());
            continue;
        }
        if current_section.is_some() {
            if !buffer.is_empty() && !buffer.ends_with('\n') {
                buffer.push('\n');
            }
            buffer.push_str(line);
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid metadata line in {}: {line}", path.display()));
        meta.insert(key.trim().to_string(), value.trim().to_string());
    }

    if let Some(section) = current_section {
        sections.insert(section, buffer.trim().to_string());
    }

    let name = meta
        .remove("name")
        .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
    let sql_file = meta
        .remove("sql_file")
        .unwrap_or_else(|| format!("{name}.sql"));
    let sql = fs::read_to_string(sql_dir.join(sql_file)).expect("failed to read SQL file");

    QuerySpec {
        name,
        family: required_meta(&meta, "family", &path),
        note: required_meta(&meta, "note", &path),
        sql,
        select: required_section(&sections, "select", &path),
        from: required_section(&sections, "from", &path),
        where_: sections
            .get("where")
            .cloned()
            .unwrap_or_else(|| "true".to_string()),
        having: sections
            .get("having")
            .cloned()
            .unwrap_or_else(|| "true".to_string()),
        groupby: sections
            .get("groupby")
            .cloned()
            .unwrap_or_else(|| "(list)".to_string()),
        orderby: sections
            .get("orderby")
            .cloned()
            .unwrap_or_else(|| "(list)".to_string()),
        limit: sections
            .get("limit")
            .cloned()
            .unwrap_or_else(|| "null".to_string()),
        offset: sections
            .get("offset")
            .cloned()
            .unwrap_or_else(|| "0".to_string()),
    }
}

fn required_meta(meta: &HashMap<String, String>, key: &str, path: &PathBuf) -> String {
    meta.get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing metadata key {key} in {}", path.display()))
}

fn required_section(
    sections: &HashMap<String, String>,
    key: &str,
    path: &PathBuf,
) -> String {
    sections
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing [{key}] section in {}", path.display()))
}

fn render_csv(results: &[QueryResult]) -> String {
    let mut csv = String::from(
        "name,family,build_ms,optimize_ms_median,optimize_ms_min,optimize_ms_max,iterations,egraph_nodes,egraph_classes,memo_size,rule_applications,best_cost,before_total_nodes,after_total_nodes,before_plan_nodes,after_plan_nodes,before_scan_columns,after_scan_columns,before_join_nodes,after_join_nodes,after_hashjoin_nodes,before_depth,after_depth,after_empty_scan_schemas,after_bare_list_symbols,after_hashjoin_empty_keys,after_hashjoin_non_column_keys,stop_reason\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},{},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            result.name,
            result.family.replace(',', " "),
            result.build_ms,
            result.optimize_ms_median,
            result.optimize_ms_min,
            result.optimize_ms_max,
            result.iterations,
            result.egraph_nodes,
            result.egraph_classes,
            result.memo_size,
            result.rule_applications,
            result.best_cost,
            result.before.total_nodes,
            result.after.total_nodes,
            result.before.plan_nodes,
            result.after.plan_nodes,
            result.before.scan_columns,
            result.after.scan_columns,
            result.before.join_nodes,
            result.after.join_nodes,
            result.after.hashjoin_nodes,
            result.before.max_depth,
            result.after.max_depth,
            result.after.empty_scan_schemas,
            result.after.bare_list_symbols,
            result.after.hashjoin_empty_keys,
            result.after.hashjoin_non_column_keys,
            result.stop_reason,
        ));
    }
    csv
}

fn render_markdown(results: &[QueryResult], samples: usize) -> String {
    let total_before_plan_nodes: usize = results.iter().map(|r| r.before.plan_nodes).sum();
    let total_after_plan_nodes: usize = results.iter().map(|r| r.after.plan_nodes).sum();
    let total_before_scan_columns: usize = results.iter().map(|r| r.before.scan_columns).sum();
    let total_after_scan_columns: usize = results.iter().map(|r| r.after.scan_columns).sum();
    let suspicious_plans = results
        .iter()
        .filter(|r| is_suspicious(&r.after))
        .count();
    let median_opt_time = median(
        &results
            .iter()
            .map(|r| r.optimize_ms_median)
            .collect::<Vec<_>>(),
    );

    let mut all_rules: BTreeMap<String, usize> = BTreeMap::new();
    for result in results {
        for (rule, count) in &result.rule_counts {
            *all_rules.entry(rule.clone()).or_default() += *count;
        }
    }
    let mut all_rules: Vec<_> = all_rules.into_iter().collect();
    all_rules.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut md = String::new();
    md.push_str("# TPC-H Subset Optimizer Evaluation\n\n");
    md.push_str("This workload measures the current project as a logical optimizer prototype on a real TPC-H subset. ");
    md.push_str("Because the repo does not yet include a full SQL parser or executor, each benchmark stores the reference SQL text plus an explicit lowered IR plan that the optimizer actually runs.\n\n");
    md.push_str("## Methodology\n\n");
    md.push_str(&format!(
        "- Workload size: {} supported-subset queries\n- Timing: median/min/max of {} optimizer runs per query\n- Optimizer configuration: `expr::rules()` + `plan::rules()` with `egg::Extractor<AstSize>`\n- Metrics: planning latency, e-graph size, rule applications, plan size reduction, and total scan-column reduction\n\n",
        results.len(),
        samples
    ));
    md.push_str("## Headline Results\n\n");
    md.push_str(&format!(
        "- Median optimizer time across workload: {:.3} ms\n- Total logical plan nodes: {} -> {} ({:.1}% reduction)\n- Total scan columns read: {} -> {} ({:.1}% reduction)\n- Queries with suspicious extracted plans: {}/{}\n\n",
        median_opt_time,
        total_before_plan_nodes,
        total_after_plan_nodes,
        percent_reduction(total_before_plan_nodes, total_after_plan_nodes),
        total_before_scan_columns,
        total_after_scan_columns,
        percent_reduction(total_before_scan_columns, total_after_scan_columns),
        suspicious_plans,
        results.len(),
    ));
    md.push_str("## Per-Query Metrics\n\n");
    md.push_str("| Query | Median ms | E-graph nodes | Rule apps | Plan nodes | Scan cols | Hash joins | Warnings |\n");
    md.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for result in results {
        md.push_str(&format!(
            "| {} | {:.3} | {} | {} | {} -> {} | {} -> {} | {} | {} |\n",
            result.name,
            result.optimize_ms_median,
            result.egraph_nodes,
            result.rule_applications,
            result.before.plan_nodes,
            result.after.plan_nodes,
            result.before.scan_columns,
            result.after.scan_columns,
            result.after.hashjoin_nodes,
            warning_summary(&result.after),
        ));
    }
    md.push_str("\n## Most Active Rules\n\n");
    md.push_str("| Rule | Applications |\n");
    md.push_str("| --- | ---: |\n");
    for (rule, count) in all_rules.into_iter().take(10) {
        md.push_str(&format!("| {} | {} |\n", rule, count));
    }
    md.push_str("\n## Query Notes\n\n");
    for result in results {
        md.push_str(&format!("### {}\n\n", result.name));
        md.push_str(&format!(
            "- Family: {}\n- Note: {}\n- Timing: {:.3} ms median, {:.3} ms min, {:.3} ms max\n- Stop reason: {}\n- Before/after plan nodes: {} -> {}\n- Before/after scan columns: {} -> {}\n- Validation warnings: {}\n\n",
            result.family,
            result.note,
            result.optimize_ms_median,
            result.optimize_ms_min,
            result.optimize_ms_max,
            result.stop_reason,
            result.before.plan_nodes,
            result.after.plan_nodes,
            result.before.scan_columns,
            result.after.scan_columns,
            warning_summary(&result.after),
        ));
        md.push_str("Reference SQL:\n\n```sql\n");
        md.push_str(&result.sql);
        if !result.sql.ends_with('\n') {
            md.push('\n');
        }
        md.push_str("```\n\nInitial plan:\n\n```text\n");
        md.push_str(&result.before_plan);
        md.push_str("\n```\n\nOptimized plan:\n\n```text\n");
        md.push_str(&result.after_plan);
        md.push_str("\n```\n\n");
    }
    md
}

fn percent_reduction(before: usize, after: usize) -> f64 {
    if before == 0 {
        0.0
    } else {
        ((before.saturating_sub(after)) as f64 / before as f64) * 100.0
    }
}

fn is_suspicious(stats: &PlanStats) -> bool {
    stats.empty_scan_schemas > 0
        || stats.bare_list_symbols > 0
        || stats.hashjoin_empty_keys > 0
        || stats.hashjoin_non_column_keys > 0
}

fn warning_summary(stats: &PlanStats) -> String {
    let mut warnings = Vec::new();
    if stats.empty_scan_schemas > 0 {
        warnings.push(format!("empty_scans={}", stats.empty_scan_schemas));
    }
    if stats.bare_list_symbols > 0 {
        warnings.push(format!("bare_list_symbols={}", stats.bare_list_symbols));
    }
    if stats.hashjoin_empty_keys > 0 {
        warnings.push(format!("hashjoin_empty_keys={}", stats.hashjoin_empty_keys));
    }
    if stats.hashjoin_non_column_keys > 0 {
        warnings.push(format!(
            "hashjoin_non_column_keys={}",
            stats.hashjoin_non_column_keys
        ));
    }
    if warnings.is_empty() {
        "none".to_string()
    } else {
        warnings.join("; ")
    }
}
