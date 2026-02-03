## Language Definition

In egg, the `EGraph` data structure is parametric to a user-defined language. For this sql-optimizer lab, the language we use is a simple language of the following components:
- Common expression nodes
- SQL plan nodes (`Scan`, `Filter`)

<!-- todo -->

## E-class Analysis

### Constant Analysis

To pass the tests in [4_constant_folding.rs](../tests/4_constant_folding.rs), we need to do constant analysis. 

<!-- todo -->

### Column-set Analysis

To pass the tests in [7_predicate_pushdown.rs](../tests/7_predicate_pushdown.rs), we need to do column-set analysis.
It's basically an analysis that tells you **which columns appear in a subtree**. For each eclass, you compute a set of columns referenced by that expression/plan node. 

It is particularly useful in, e.g. predicate pushdown:

- For example, when you have a predicate like `s.name <> 'Alice'`, you want to push it **only** into the `scan s` side, not into `scan e`.
- So you need to check: "does this predicate mention only columns from the left subtree"?


