# SQL Optimizer Labs

Build a SQL optimizer in 1000 lines of Rust using [egg](https://egraphs-good.github.io).

🚧 Under construction 🚧 Stay tuned 👀

For explanation of implementation, please refer [src/README.md](src/README.md).

For tutorials (in Chinese), please refer [欢度元旦，用蛋写个 SQL 优化器吧（上）
](https://zhuanlan.zhihu.com/p/596119553) and [欢度元宵，用蛋写个 SQL 优化器吧！
](https://zhuanlan.zhihu.com/p/604073131).

## Tasks

Fill the code in `src` and pass the tests in `tests`!

```sh
cargo test --test 1_language
cargo test --test 2_rewrite
cargo test --test 3_conditional_rewrite
cargo test --test 4_constant_folding
cargo test --test 5_sql_plan
cargo test --test 6_plan_elimination
cargo test --test 7_predicate_pushdown
cargo test --test 8_projection_pushdown
cargo test --test 9_agg_extraction
cargo test --test 10_index_resolving
```

## What's Next

These labs are taken from the [RisingLight] project.
[Check out] how it works in a real database system!

[RisingLight]: https://github.com/risinglightdb/risinglight
[Check out]: https://github.com/risinglightdb/risinglight/blob/main/src/planner/mod.rs
