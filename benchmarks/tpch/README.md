# TPC-H Subset Benchmark

This directory contains a real TPC-H subset used to evaluate the optimizer at the logical-plan level.

## Layout

- `sql/`: reference SQL text for each supported TPC-H query
- `specs/`: explicit lowered IR that the optimizer actually runs
- `results.csv`: generated benchmark metrics
- `results.md`: generated benchmark report

## Scope

The current repo does not include a full SQL parser or an execution engine, so this benchmark is intentionally split into:

1. The original TPC-H SQL text for credibility and inspection.
2. A hand-lowered IR plan for reproducible optimizer evaluation.

This means the project should be described as a `TPC-H subset logical optimizer evaluation`, not a fully compliant TPC-H performance run.

## Reproduce

Run:

```sh
cargo run --bin tpch_bench -- --samples 3 --csv benchmarks/tpch/results.csv --markdown benchmarks/tpch/results.md
```

The markdown report includes per-query warnings for suspicious extracted plans, such as empty scan schemas or invalid hash-join keys. Those warnings should be treated as optimizer limitations, not benchmark wins.

## Supported Queries

- `Q1`
- `Q3`
- `Q5`
- `Q6`
- `Q10`

## Normalizations

- Date arithmetic from SQL templates is pre-evaluated into integer date literals in the IR.
- `Q6` discount ranges are represented as integer percentage points in the IR.
- `Q1` uses `count(l_quantity)` in the IR because the toy language does not have `count(*)`.
