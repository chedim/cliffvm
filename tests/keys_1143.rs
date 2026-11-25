//! `keys(d)`: the keys of a dict, as a list of text, in the order the dict holds them (#1143, operator 2026-11-16: the one builtin of the old "proposed, not added" list that is added).
//! It is pure (a value in, a list out), costs one step whatever the size, and a read still pending is an empty list, as `pluck` does.
//! What it gives and where it fails are tests/c1ft/keys.c1ft; what stays here is the step budget and the reserved name, which c1ft cannot say.
use cliffvm::expr::{self, V};
use serde_json::{json, Value as J};
use std::collections::HashMap;

struct W(HashMap<(String, String), V>);
impl expr::World for W {
    fn attr(&self, eid: &str, attr: &str) -> V { self.0.get(&(eid.to_string(), attr.to_string())).cloned().unwrap_or_else(|| V::absent(format!("{eid}.{attr}"))) }
}
fn world() -> W {
    let mut m = HashMap::new();
    m.insert(("A:1".to_string(), "d".to_string()), expr::from_json(&json!({"k": "one", "j": 2})));
    m.insert(("A:1".to_string(), "list".to_string()), expr::from_json(&json!([1, 2])));
    W(m)
}

#[test]
fn it_costs_a_handful_of_steps_whatever_the_size_of_the_dict() {
    // the dict comes in as a variable, so building it is not counted: only the call is
    let big: J = J::Object((0..5000).map(|i| (format!("k{i}"), json!(i))).collect());
    let n = expr::parse("keys(d)").unwrap();
    let env: HashMap<String, V> = [("d".to_string(), expr::from_json(&big))].into_iter().collect();
    expr::set_eval_budget(Some(8));
    let r = expr::eval(&n, &env, &world());
    expr::set_eval_budget(None);
    match r { Ok(V::List(l)) => assert_eq!(l.len(), 5000), other => panic!("under a budget of 8 steps: {other:?}") }
}

#[test]
fn keys_is_a_reserved_name_like_the_other_builtins() {
    assert!(expr::RESERVED.contains(&"keys"));
}
