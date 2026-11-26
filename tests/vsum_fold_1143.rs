//! Two builtins the language could not say (#1143, operator 2026-11-15):
//! `vsum(vectors, weights?)`, the (weighted) sum of a LIST of vectors (`+` adds two; nothing added up a list), and
//! `fold(init, [step for x in xs if keep])`, a comprehension that carries a value across the list: `acc` is `init`, then each step's value
//! (a greedy pass keeps state, e.g. "keep a hit only if it is not a near duplicate of a hit ALREADY KEPT").
use cliffvm::expr;
use serde_json::{json, Value as J};

fn ev(src: &str) -> Result<J, String> { cliffvm::evaluate_json(src, "{}").map(|s| serde_json::from_str(&s).unwrap()) }
fn ev_env(src: &str, env: J) -> Result<J, String> { cliffvm::evaluate_json(src, &env.to_string()).map(|s| serde_json::from_str(&s).unwrap()) }

// ---- vsum ----------------------------------------------------------------------------------------------------------

#[test]
fn vsum_is_f32_math_in_list_order() {
    // 0.1 as an f32 is 0.100000001490116...; the result is that f32, read back as a float
    assert_eq!(ev("vsum([[0.1]])").unwrap(), json!([0.1f32 as f64]));
    // in list order: (1e8 + 1) - 1e8 in f32 is 0 (1e8 + 1 rounds to 1e8), not 1
    assert_eq!(ev("vsum([[100000000.0], [1.0], [-100000000.0]])").unwrap(), json!([0.0]));
}

#[test]
fn vsum_is_one_step_however_many_vectors() {
    let vs: Vec<J> = (0..2000).map(|i| json!([i, 1])).collect();
    let n = expr::parse("vsum(vs)").unwrap();
    let env = [("vs".to_string(), expr::from_json(&J::Array(vs)))].into_iter().collect();
    struct W; impl expr::World for W { fn attr(&self, _: &str, _: &str) -> expr::V { expr::V::None } }
    expr::set_eval_budget(Some(5));
    let r = expr::eval(&n, &env, &W);
    expr::set_eval_budget(None);
    assert!(r.is_ok(), "{r:?}");
}

// ---- fold ----------------------------------------------------------------------------------------------------------

#[test]
fn acc_is_bound_only_inside_a_fold_and_hides_a_name_of_its_own_there() {
    assert!(ev("acc + 1").is_err());
    assert!(expr::RESERVED.contains(&"fold") && expr::RESERVED.contains(&"vsum"));
    // `acc` is NOT reserved (a shipped pack has a local of that name): inside a step the fold's own `acc` is meant
    assert_eq!(ev_env("[acc, fold(0, [acc + x for x in [1, 2]])]", json!({"acc": "outer"})).unwrap(), json!(["outer", 3]));
    assert!(ev("fold(0, [acc for acc in [1]])").unwrap_err().contains("acc"), "the loop's item is not named acc");
}

#[test]
fn an_error_in_a_step_is_the_answer() {
    // a failed step stops the fold: its Error is the value (as an Error passes through every operation)
    let n = expr::parse("fold(0, [acc + Error('bad step') for x in [1, 2]])").unwrap();
    struct W; impl expr::World for W { fn attr(&self, _: &str, _: &str) -> expr::V { expr::V::None } }
    let r = expr::eval(&n, &Default::default(), &W).unwrap();
    assert!(matches!(&r, expr::V::Error(m) if m == "bad step"), "{r:?}");
}

#[test]
fn the_steps_of_a_fold_count_against_the_budget() {
    let n = expr::parse("fold(0, [acc + x for x in xs])").unwrap();
    let xs: Vec<J> = (0..1000).map(|i| json!(i)).collect();
    let env = [("xs".to_string(), expr::from_json(&J::Array(xs)))].into_iter().collect();
    struct W; impl expr::World for W { fn attr(&self, _: &str, _: &str) -> expr::V { expr::V::None } }
    expr::set_eval_budget(Some(500));
    let r = expr::eval(&n, &env, &W);
    expr::set_eval_budget(None);
    assert_eq!(r.unwrap_err(), "budget exceeded (500 steps)");
    let r = expr::eval(&n, &env, &W);
    assert_eq!(expr::to_json(&r.unwrap()).unwrap(), json!(499500));
}

#[test]
fn the_inferencer_types_vsum_and_checks_the_arity_of_both() {
    use cliffvm::infer::infer_src;
    assert_eq!(infer_src("vsum(v)", &[("v", "List(List(Float))")]).unwrap().0, "List(Float)");
    assert!(!infer_src("vsum(v, w, w)", &[("v", "List(List(Float))"), ("w", "List(Float)")]).unwrap().1.is_empty(), "three arguments are reported");
    assert!(!infer_src("fold(0)", &[]).unwrap().1.is_empty(), "one argument is reported");
    assert!(infer_src("fold(0, [acc + x for x in [1, 2]])", &[]).unwrap().1.is_empty());
}
