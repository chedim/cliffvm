//! `keys(d)`: the keys of a dict, as a list of text, in the order the dict holds them (#1143, operator 2026-11-16: the one builtin of the old "proposed, not added" list that is added).
//! It is pure (a value in, a list out), costs one step whatever the size, and a read still pending is an empty list, as `pluck` does.
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
fn run(src: &str) -> Result<V, String> {
    let n = expr::parse(src).map_err(|e| e.to_string())?;
    expr::eval(&n, &HashMap::new(), &world())
}
fn ok(src: &str) -> J { expr::to_json(&run(src).unwrap_or_else(|e| panic!("{src}: {e}"))).unwrap() }
fn err(src: &str) -> String { match run(src) { Err(e) => e, Ok(V::Error(e)) => e.text(), Ok(v) => panic!("{src}: not an error: {v:?}") } }

#[test]
fn the_keys_of_a_dict_come_in_the_order_it_holds_them() {
    assert_eq!(ok("keys({'b': 1, 'a': 2, 'c': 3})"), json!(["b", "a", "c"]));
    assert_eq!(ok("keys({'a': 2, 'b': 1})"), json!(["a", "b"]), "a reorder is a different order");
    assert_eq!(ok("keys({})"), json!([]));
}

#[test]
fn it_reads_a_dict_held_by_an_entity_and_composes_with_the_other_builtins() {
    assert_eq!(ok("keys(prop('A:1', 'd'))"), json!(["k", "j"]));
    assert_eq!(ok("len(keys(prop('A:1', 'd')))"), json!(2));
    assert_eq!(ok("[prop('A:1', 'd')[k] for k in keys(prop('A:1', 'd'))]"), json!(["one", 2]));
    assert_eq!(ok("sort_by([{'k': k} for k in keys({'b': 1, 'a': 2})], 'k')"), json!([{"k": "a"}, {"k": "b"}]));
}

#[test]
fn a_read_that_has_not_arrived_is_an_empty_list_and_anything_else_is_an_error() {
    assert_eq!(ok("keys(prop('A:9', 'd'))"), json!([]), "an absent property is a pending read: nothing to list");
    assert!(err("keys([1, 2])").contains("keys"), "a list has no keys");
    assert_eq!(ok("keys('text')"), json!([]), "a text is an entity id now: no such entity, no names (it was an error before an entity could be named)");
    assert!(err("keys(1)").contains("keys"), "a number has no keys");
    assert!(err("keys(1, 2)").contains("keys"), "one argument");
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

// ---- an entity (operator 2026-11-24): its property names, the getters' included, from memory -------------------------------------------------------------------------
struct E(HashMap<(String, String), V>);
impl expr::World for E {
    fn attr(&self, eid: &str, attr: &str) -> V { self.0.get(&(eid.to_string(), attr.to_string())).cloned().unwrap_or_else(|| V::absent(format!("{eid}.{attr}"))) }
    fn prop_names(&self, eid: &str) -> Option<Vec<String>> {
        let mut v: Vec<String> = self.0.keys().filter(|(e, _)| e == eid).map(|(_, p)| p.clone()).collect();
        v.sort();
        if v.is_empty() { None } else { Some(v) }
    }
}
fn entity_world() -> E {
    let mut m = HashMap::new();
    m.insert(("A:1".to_string(), "n".to_string()), V::Int(1));
    m.insert(("A:1".to_string(), "twice".to_string()), V::Int(2));
    E(m)
}
fn run_e(src: &str) -> V { let n = expr::parse(src).unwrap(); expr::eval(&n, &HashMap::new(), &entity_world()).unwrap() }

#[test]
fn the_keys_of_an_entity_are_the_names_of_its_properties_the_getters_included() {
    assert_eq!(expr::to_json(&run_e("keys('A:1')")).unwrap(), json!(["n", "twice"]));
    assert_eq!(expr::to_json(&run_e("keys('A:9')")).unwrap(), json!([]), "no such entity: no names");
    assert_eq!(expr::to_json(&run_e("[k + '=' + str(prop('A:1', k)) for k in keys('A:1')]")).unwrap(), json!(["n=1", "twice=2"]), "and each one reads");
}
