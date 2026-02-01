use sql_optimizer_labs::expr::rules;

egg::test_fn! {
    #[should_panic]
    mul_div_0,
    rules(),
    "(/ (* a 0) 0)" => "a",
}

egg::test_fn! {
    mul_div,
    rules(),
    "(/ (* a 2) 2)" => "a",
}

egg::test_fn! {
    #[should_panic]
    mul_div_unknown_denominator,
    rules(),
    "(/ (* a b) b)" => "a",
}

egg::test_fn! {
    #[should_panic]
    mul_div_denominator_simplifies,
    rules(),
    "(/ (* a (+ b 0)) (+ b 0))" => "a",
}
