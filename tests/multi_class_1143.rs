//! An entity of TWO extra classes at once (#1143). Its classes are the list in its `parents` property (`cls:<name>` ids, the ISA kinds): the
//! getters of both apply, the setter of each fires (a kind's setter needs its ISA parent entity `cls:<name>`), and the list survives a
//! journal replay. What the getters and setters do is tests/c1ft/multi_class.c1ft; what stays here is the replay.
//! A `parents` write on an entity that already exists binds the getters of its kinds (they are bound at birth otherwise).
use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};

const TWO: &str = "PACK two 1\nAlpha.sa getter: 'alpha:' + str(this.n)\nBeta.sb getter: 'beta:' + str(this.n)\nAlpha.go_a on-setter run-a:\n  WHEN this.go_a == True:\n    this.did_a = True\nBeta.go_b on-setter run-b:\n  WHEN this.go_b == True:\n    this.did_b = True\n";

fn fresh() -> Host {
    let mut o = Options::new(None);
    o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("two", TWO).unwrap();
    h.rebuild();
    h
}
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }

#[test]
fn the_class_list_survives_a_journal_replay() {
    let h = fresh();
    h.born("cls:alpha", "cls", props(json!({"name": "Alpha"})), "person");
    h.born("cls:beta", "cls", props(json!({"name": "Beta"})), "person");
    h.born("Thing:x", "Thing", props(json!({"n": 3})), "person");
    h.write("Thing:x", "parents", json!(["cls:alpha", "cls:beta"]), "person");
    h.write("Thing:x", "n", json!(4), "person");
    h.write("Thing:x", "go_a", json!(true), "person");
    h.write("Thing:x", "go_b", json!(true), "person");
    // a journal replay into a fresh engine gives the same entity
    let r = fresh();
    r.replay(&h.journal());
    assert_eq!(r.snapshot()["Thing:x"], h.snapshot()["Thing:x"]);
}
