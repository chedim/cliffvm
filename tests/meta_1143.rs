//! `meta(eid)`: the metadata of an entity as a dict, names only, no values (#1143, operator 2026-11-24). It replaces `keys(entity)`: `entity_read` is this dict printed.
//! `class`, `parents` (the class list, nearest first), `properties` (what the entity holds, a getter's value not among them), `getters` and `setters` (those of its classes).
mod common;

use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};

fn seat() -> Host {
    let mut o = Options::new(None);
    o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("mt", PACK).unwrap();
    h.rebuild();
    h
}
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }

const PACK: &str = "PACK mt 1
Thing.twice getter: this.n * 2
Thing.shown getter: meta(this.target)
Thing.missing getter: meta('Thing:nope') == None and (why(meta('Thing:nope')) is Absent)
Thing.go on-setter bump:
  ALWAYS:
    this.hit = 1
";

#[test]
fn it_names_the_class_the_properties_the_getters_and_the_setters_and_prints_no_value() {
    let h = seat();
    h.born("Thing:t1", "Thing", props(json!({"n": 4, "secret": "do not print"})), "person");
    h.born("Thing:t2", "Thing", props(json!({"target": "Thing:t1"})), "person");
    let m = h.get("Thing:t2", "shown");
    assert_eq!(m["class"], json!("Thing"), "{m}");
    let names = |k: &str| -> Vec<String> { let mut v: Vec<String> = m[k].as_array().unwrap_or_else(|| panic!("{k}: {m}")).iter().map(|x| x.as_str().unwrap().to_string()).collect(); v.sort(); v };
    assert_eq!(names("properties"), ["n", "secret"], "what it holds, not what its getters computed: {m}");
    assert_eq!(names("getters"), ["missing", "shown", "twice"], "{m}");
    assert!(m["setters"].as_array().unwrap().iter().any(|s| s["prop"] == json!("go") && s["name"] == json!("bump")), "{m}");
    assert!(!m.to_string().contains("do not print"), "no value: {m}");
    assert!(m["parents"].is_array(), "{m}");
}

#[test]
fn a_text_that_names_no_entity_is_a_null() {
    let h = seat();
    h.born("Thing:t3", "Thing", props(json!({"target": "Thing:nope"})), "person");
    let m = h.get("Thing:t3", "shown");
    assert!(m.is_null(), "{m}");
}

#[test]
fn an_entity_that_is_not_there_says_so() {
    let h = seat();
    h.born("Thing:t5", "Thing", props(json!({"n": 1})), "person");
    assert_eq!(h.get("Thing:t5", "missing"), json!(true), "a null whose reason is Absent: the door answers `no such entity`, it does not wait");
}
