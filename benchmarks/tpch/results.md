# TPC-H Subset Optimizer Evaluation

This workload measures the current project as a logical optimizer prototype on a real TPC-H subset. Because the repo does not yet include a full SQL parser or executor, each benchmark stores the reference SQL text plus an explicit lowered IR plan that the optimizer actually runs.

## Methodology

- Workload size: 5 supported-subset queries
- Timing: median/min/max of 3 optimizer runs per query
- Optimizer configuration: `expr::rules()` + `plan::rules()` with `egg::Extractor<AstSize>`
- Metrics: planning latency, e-graph size, rule applications, plan size reduction, and total scan-column reduction

## Headline Results

- Median optimizer time across workload: 1323.862 ms
- Total logical plan nodes: 52 -> 48 (7.7% reduction)
- Total scan columns read: 97 -> 59 (39.2% reduction)
- Queries with suspicious extracted plans: 4/5

## Per-Query Metrics

| Query | Median ms | E-graph nodes | Rule apps | Plan nodes | Scan cols | Hash joins | Warnings |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| q1 | 5.809 | 197 | 104 | 6 -> 5 | 15 -> 15 | 0 | none |
| q10 | 1441.387 | 51292 | 19537 | 13 -> 13 | 22 -> 12 | 1 | empty_scans=1; hashjoin_non_column_keys=1 |
| q3 | 1323.862 | 43190 | 17097 | 11 -> 11 | 21 -> 0 | 1 | empty_scans=3; hashjoin_empty_keys=1; hashjoin_non_column_keys=1 |
| q5 | 2246.385 | 50801 | 22334 | 16 -> 14 | 28 -> 28 | 2 | hashjoin_non_column_keys=1 |
| q6 | 9.953 | 414 | 273 | 6 -> 5 | 11 -> 4 | 0 | bare_list_symbols=1 |

## Most Active Rules

| Rule | Applications |
| --- | ---: |
| pushdown-proj-join | 10911 |
| pushdown-proj-filter | 8738 |
| filter-true | 5406 |
| column-prune | 4522 |
| pushdown-proj-scan | 4276 |
| pushdown-filter-over-inner-join-left | 3978 |
| pushdown-filter-over-inner-join-right | 3850 |
| column-merge | 3523 |
| hash-join-on-one-eq | 3013 |
| proj-merge | 2816 |

## Query Notes

### q1

- Family: TPC-H Q1 Pricing Summary Report Query
- Note: Lowered from the real TPC-H Q1 template. The toy IR does not support count(*), so the lowering uses count(l_quantity); that is equivalent under the standard non-null TPC-H schema.
- Timing: 5.809 ms median, 5.643 ms min, 5.893 ms max
- Stop reason: Some(Saturated)
- Before/after plan nodes: 6 -> 5
- Before/after scan columns: 15 -> 15
- Validation warnings: none

Reference SQL:

```sql
select
    l_returnflag,
    l_linestatus,
    sum(l_quantity) as sum_qty,
    sum(l_extendedprice) as sum_base_price,
    sum(l_extendedprice * (1 - l_discount)) as sum_disc_price,
    sum(l_extendedprice * (1 - l_discount) * (1 + l_tax)) as sum_charge,
    avg(l_quantity) as avg_qty,
    avg(l_extendedprice) as avg_price,
    avg(l_discount) as avg_disc,
    count(*) as count_order
from
    lineitem
where
    l_shipdate <= date '1998-09-02'
group by
    l_returnflag,
    l_linestatus
order by
    l_returnflag,
    l_linestatus;
```

Initial plan:

```text
(proj (list (` l_returnflag) (` l_linestatus) (` (sum l_quantity)) (` (sum l_extendedprice)) (` (sum (* l_extendedprice (- 1 l_discount)))) (` (sum (* (* l_extendedprice (- 1 l_discount)) (+ 1 l_tax)))) (` (avg l_quantity)) (` (avg l_extendedprice)) (` (avg l_discount)) (` (count l_quantity))) (order (list (asc (` l_returnflag)) (asc (` l_linestatus))) (filter true (agg (list (sum l_quantity) (sum l_extendedprice) (sum (* l_extendedprice (- 1 l_discount))) (sum (* (* l_extendedprice (- 1 l_discount)) (+ 1 l_tax))) (avg l_quantity) (avg l_extendedprice) (avg l_discount) (count l_quantity)) (list l_returnflag l_linestatus) (filter (<= l_shipdate 19980902) (scan lineitem (list l_orderkey l_partkey l_suppkey l_linenumber l_quantity l_extendedprice l_discount l_tax l_returnflag l_linestatus l_shipdate l_commitdate l_receiptdate l_shipinstruct l_shipmode)))))))
```

Optimized plan:

```text
(proj (list (` l_returnflag) (` l_linestatus) (` (sum l_quantity)) (` (sum l_extendedprice)) (` (sum (* l_extendedprice (- 1 l_discount)))) (` (sum (* l_extendedprice (* (+ l_tax 1) (- 1 l_discount))))) (` (avg l_quantity)) (` (avg l_extendedprice)) (` (avg l_discount)) (` (count l_quantity))) (order (list (asc (` l_returnflag)) (asc (` l_linestatus))) (agg (list (sum l_quantity) (sum l_extendedprice) (sum (* l_extendedprice (- 1 l_discount))) (sum (* l_extendedprice (* (+ l_tax 1) (- 1 l_discount)))) (avg l_quantity) (avg l_extendedprice) (avg l_discount) (count l_quantity)) (list l_returnflag l_linestatus) (filter (>= 19980902 l_shipdate) (scan lineitem (list l_orderkey l_partkey l_suppkey l_linenumber l_quantity l_extendedprice l_discount l_tax l_returnflag l_linestatus l_shipdate l_commitdate l_receiptdate l_shipinstruct l_shipmode))))))
```

### q10

- Family: TPC-H Q10 Returned Item Reporting Query
- Note: Direct lowering of TPC-H Q10 with the standard 1993-10-01 to 1994-01-01 date window and limit 20.
- Timing: 1441.387 ms median, 1439.532 ms min, 1549.337 ms max
- Stop reason: Some(NodeLimit(51292))
- Before/after plan nodes: 13 -> 13
- Before/after scan columns: 22 -> 12
- Validation warnings: empty_scans=1; hashjoin_non_column_keys=1

Reference SQL:

```sql
select
    c_custkey,
    c_name,
    sum(l_extendedprice * (1 - l_discount)) as revenue,
    c_acctbal,
    n_name,
    c_address,
    c_phone,
    c_comment
from
    customer,
    orders,
    lineitem,
    nation
where
    c_custkey = o_custkey
    and l_orderkey = o_orderkey
    and o_orderdate >= date '1993-10-01'
    and o_orderdate < date '1994-01-01'
    and l_returnflag = 'R'
    and c_nationkey = n_nationkey
group by
    c_custkey,
    c_name,
    c_acctbal,
    c_phone,
    n_name,
    c_address,
    c_comment
order by
    revenue desc
limit 20;
```

Initial plan:

```text
(limit 20 0 (proj (list (` c_custkey) (` c_name) (` (sum (* l_extendedprice (- 1 l_discount)))) (` c_acctbal) (` n_name) (` c_address) (` c_phone) (` c_comment)) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount)))))) (filter true (agg (list (sum (* l_extendedprice (- 1 l_discount)))) (list c_custkey c_name c_acctbal c_phone n_name c_address c_comment) (filter (and (>= o_orderdate 19931001) (and (< o_orderdate 19940101) (= l_returnflag 'R'))) (join inner (= c_nationkey n_nationkey) (join inner (= c_custkey o_custkey) (scan customer (list c_custkey c_name c_address c_nationkey c_phone c_acctbal c_comment c_mktsegment)) (join inner (= o_orderkey l_orderkey) (scan orders (list o_orderkey o_custkey o_orderdate o_totalprice o_orderstatus)) (scan lineitem (list l_orderkey l_returnflag l_extendedprice l_discount l_shipdate l_quantity)))) (scan nation (list n_nationkey n_name n_regionkey)))))))))
```

Optimized plan:

```text
(limit 20 0 (proj (list (` c_custkey) (` c_name) (` (sum (* l_extendedprice (- 1 l_discount)))) (` c_acctbal) (` n_name) (` c_address) (` c_phone) (` c_comment)) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount)))))) (agg (list (sum (* l_extendedprice (- 1 l_discount)))) (list c_custkey c_name c_acctbal c_phone n_name c_address c_comment) (join inner (= n_nationkey c_nationkey) (filter (> 19940101 o_orderdate) (filter (>= o_orderdate 19931001) (join inner (= o_custkey c_custkey) (scan customer (list c_custkey c_name c_address c_nationkey c_phone c_acctbal c_comment)) (hashjoin inner (list o_orderkey 'R') (list l_orderkey l_returnflag) (scan orders list) (scan lineitem (list l_extendedprice l_discount)))))) (scan nation (list n_nationkey n_name n_regionkey)))))))
```

### q3

- Family: TPC-H Q3 Shipping Priority Query
- Note: Direct lowering of TPC-H Q3 with the standard BUILDING segment and 1995-03-15 date parameter.
- Timing: 1323.862 ms median, 1317.557 ms min, 1331.676 ms max
- Stop reason: Some(Saturated)
- Before/after plan nodes: 11 -> 11
- Before/after scan columns: 21 -> 0
- Validation warnings: empty_scans=3; hashjoin_empty_keys=1; hashjoin_non_column_keys=1

Reference SQL:

```sql
select
    l_orderkey,
    sum(l_extendedprice * (1 - l_discount)) as revenue,
    o_orderdate,
    o_shippriority
from
    customer,
    orders,
    lineitem
where
    c_mktsegment = 'BUILDING'
    and c_custkey = o_custkey
    and l_orderkey = o_orderkey
    and o_orderdate < date '1995-03-15'
    and l_shipdate > date '1995-03-15'
group by
    l_orderkey,
    o_orderdate,
    o_shippriority
order by
    revenue desc,
    o_orderdate
limit 10;
```

Initial plan:

```text
(limit 10 0 (proj (list (` l_orderkey) (` (sum (* l_extendedprice (- 1 l_discount)))) (` o_orderdate) (` o_shippriority)) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount))))) (asc (` o_orderdate))) (filter true (agg (list (sum (* l_extendedprice (- 1 l_discount)))) (list l_orderkey o_orderdate o_shippriority) (filter (and (= c_mktsegment 'BUILDING') (and (< o_orderdate 19950315) (> l_shipdate 19950315))) (join inner (= o_orderkey l_orderkey) (join inner (= c_custkey o_custkey) (scan customer (list c_custkey c_mktsegment c_name c_address c_nationkey c_phone c_acctbal)) (scan orders (list o_orderkey o_custkey o_orderdate o_shippriority o_totalprice o_orderstatus))) (scan lineitem (list l_orderkey l_extendedprice l_discount l_shipdate l_commitdate l_quantity l_partkey l_suppkey)))))))))
```

Optimized plan:

```text
(limit 10 0 (proj (list (` l_orderkey) (` (sum (* l_extendedprice (- 1 l_discount)))) (` o_orderdate) (` o_shippriority)) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount))))) (asc (` o_orderdate))) (agg (list (sum (* l_extendedprice (- 1 l_discount)))) list (join inner (= l_orderkey o_orderkey) (filter (> 19950315 o_orderdate) (hashjoin inner list (list o_custkey 'BUILDING') (scan customer list) (scan orders list))) (filter (> l_shipdate 19950315) (scan lineitem list)))))))
```

### q5

- Family: TPC-H Q5 Local Supplier Volume Query
- Note: Direct lowering of TPC-H Q5 with the ASIA region and the 1994 one-year orderdate window.
- Timing: 2246.385 ms median, 2170.531 ms min, 2290.522 ms max
- Stop reason: Some(NodeLimit(50801))
- Before/after plan nodes: 16 -> 14
- Before/after scan columns: 28 -> 28
- Validation warnings: hashjoin_non_column_keys=1

Reference SQL:

```sql
select
    n_name,
    sum(l_extendedprice * (1 - l_discount)) as revenue
from
    customer,
    orders,
    lineitem,
    supplier,
    nation,
    region
where
    c_custkey = o_custkey
    and l_orderkey = o_orderkey
    and l_suppkey = s_suppkey
    and c_nationkey = s_nationkey
    and s_nationkey = n_nationkey
    and n_regionkey = r_regionkey
    and r_name = 'ASIA'
    and o_orderdate >= date '1994-01-01'
    and o_orderdate < date '1995-01-01'
group by
    n_name
order by
    revenue desc;
```

Initial plan:

```text
(proj (list (` n_name) (` (sum (* l_extendedprice (- 1 l_discount))))) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount)))))) (filter true (agg (list (sum (* l_extendedprice (- 1 l_discount)))) (list n_name) (filter (and (= c_nationkey s_nationkey) (and (= r_name 'ASIA') (and (>= o_orderdate 19940101) (< o_orderdate 19950101)))) (join inner (= n_regionkey r_regionkey) (join inner (= s_nationkey n_nationkey) (join inner (= l_suppkey s_suppkey) (join inner (= o_orderkey l_orderkey) (join inner (= c_custkey o_custkey) (scan customer (list c_custkey c_nationkey c_name c_phone c_acctbal c_address)) (scan orders (list o_orderkey o_custkey o_orderdate o_totalprice o_orderstatus))) (scan lineitem (list l_orderkey l_suppkey l_extendedprice l_discount l_shipdate l_quantity))) (scan supplier (list s_suppkey s_name s_nationkey s_phone s_acctbal))) (scan nation (list n_nationkey n_name n_regionkey))) (scan region (list r_regionkey r_name r_comment))))))))
```

Optimized plan:

```text
(proj (list (` n_name) (` (sum (* l_extendedprice (- 1 l_discount))))) (order (list (desc (` (sum (* l_extendedprice (- 1 l_discount)))))) (agg (list (sum (* l_extendedprice (- 1 l_discount)))) (list n_name) (hashjoin inner (list c_custkey c_nationkey) (list o_custkey s_nationkey) (scan customer (list c_custkey c_nationkey c_name c_phone c_acctbal c_address)) (join inner (and (= n_nationkey s_nationkey) (and (> 19950101 o_orderdate) (>= o_orderdate 19940101))) (join inner (= l_orderkey o_orderkey) (scan orders (list o_orderkey o_custkey o_orderdate o_totalprice o_orderstatus)) (join inner (= s_suppkey l_suppkey) (scan lineitem (list l_orderkey l_suppkey l_extendedprice l_discount l_shipdate l_quantity)) (scan supplier (list s_suppkey s_name s_nationkey s_phone s_acctbal)))) (hashjoin inner (list n_regionkey 'ASIA') (list r_regionkey r_name) (scan nation (list n_nationkey n_name n_regionkey)) (scan region (list r_regionkey r_name r_comment))))))))
```

### q6

- Family: TPC-H Q6 Forecasting Revenue Change Query
- Note: Lowered from the real TPC-H Q6 template. Decimal discounts are represented as integer percentage points in the toy IR.
- Timing: 9.953 ms median, 9.830 ms min, 10.515 ms max
- Stop reason: Some(Saturated)
- Before/after plan nodes: 6 -> 5
- Before/after scan columns: 11 -> 4
- Validation warnings: bare_list_symbols=1

Reference SQL:

```sql
select
    sum(l_extendedprice * l_discount) as revenue
from
    lineitem
where
    l_shipdate >= date '1994-01-01'
    and l_shipdate < date '1995-01-01'
    and l_discount between 0.05 and 0.07
    and l_quantity < 24;
```

Initial plan:

```text
(proj (list (` (sum (* l_extendedprice l_discount)))) (order list (filter true (agg (list (sum (* l_extendedprice l_discount))) list (filter (and (and (>= l_shipdate 19940101) (< l_shipdate 19950101)) (and (and (>= l_discount 5) (<= l_discount 7)) (< l_quantity 24))) (scan lineitem (list l_orderkey l_partkey l_suppkey l_linenumber l_quantity l_extendedprice l_discount l_tax l_shipdate l_commitdate l_receiptdate)))))))
```

Optimized plan:

```text
(proj (list (` (sum (* l_discount l_extendedprice)))) (agg (list (sum (* l_discount l_extendedprice))) list (proj (list l_discount l_extendedprice list) (filter (and (> 24 l_quantity) (and (>= l_discount 5) (and (>= l_shipdate 19940101) (and (>= 7 l_discount) (> 19950101 l_shipdate))))) (scan lineitem (list l_quantity l_extendedprice l_discount l_shipdate))))))
```

