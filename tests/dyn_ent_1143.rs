//! `${expr}`: the entity whose id is the value of `expr` (#1143, operator 2026-11-24). An id that arrives as data (`this.target`) is read like an entity literal: `${this.target}[this.prop]`
//! or `${this.target}.status`. It is what `prop(id, name)` did, written as a value: `[name]` and `.name` after it read a property, so the builtins `prop` and `props` are gone.
use cliffvm::expr::{self, V};
use serde_json::{json, Value as J};
use std::collections::HashMap;

struct W(HashMap<(String, String), V>);
impl expr::World for W {
    fn attr(&self, eid: &str, attr: &str) -> V { self.0.get(&(eid.to_string(), attr.to_string())).cloned().unwrap_or_else(|| V::absent(format!("{eid}.{attr}"))) }
}
fn world() -> W {
    let mut m = HashMap::new();
    m.insert(("Command:x".to_string(), "status".to_string()), V::Str("ok".into()));
    m.insert(("Command:x".to_string(), "rc".to_string()), V::Int(0));
    W(m)
}
fn run(src: &str) -> Result<V, String> {
    let n = expr::parse(src).map_err(|e| e.to_string())?;
    let env: HashMap<String, V> = [("t".to_string(), V::Str("Command:x".into())), ("p".to_string(), V::Str("rc".into())), ("none".to_string(), V::None), ("n".to_string(), V::Int(3))].into_iter().collect();
    expr::eval(&n, &env, &world())
}
fn ok(src: &str) -> J { expr::to_json(&run(src).unwrap_or_else(|e| panic!("{src}: {e}"))).unwrap() }

#[test]
fn a_text_in_braces_is_the_entity_it_names_and_a_key_or_a_dot_reads_its_property() {
    assert_eq!(ok("${'Command:x'}['status']"), json!("ok"));
    assert_eq!(ok("${t}.status"), json!("ok"));
    assert_eq!(ok("${t}[p]"), json!(0), "the class and the property are both data");
    assert_eq!(ok("[${t}[k] for k in ['status', 'rc']]"), json!(["ok", 0]));
}

#[test]
fn a_property_the_entity_lacks_is_a_null_and_a_null_id_is_a_null() {
    assert!(ok("${t}.nope").is_null());
    assert!(ok("${none}[p]").is_null(), "an id that is not there reads nothing");
}

#[test]
fn only_a_text_names_an_entity_and_a_dollar_alone_is_not_an_expression() {
    assert!(run("${n}.x").is_err(), "a number is not an id");
    assert!(run("$x").is_err());
    assert_eq!(ok("'${t}'"), json!("${t}"), "inside quotes it is text, as always");
}

#[test]
fn prop_and_props_are_not_builtins_any_more() {
    assert!(run("prop('Command:x', 'status')").unwrap_err().contains("built-ins"), "`${{id}}[name]` reads it now");
    assert!(run("props(['Command:x'], 'status')").unwrap_err().contains("built-ins"));
}

#[test]
fn a_dict_in_braces_stays_the_dict_so_the_rewrite_of_a_dict_read_is_one_to_one() {
    assert_eq!(ok("${{'a': 1}}['a']"), json!(1), "prop(d, 'a') on a dict read its key");
    assert_eq!(ok("[${r}['a'] for r in [{'a': 1}, {'a': 2}]]"), json!([1, 2]));
}
