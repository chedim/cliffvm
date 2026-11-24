//! A getter's value is ephemeral (operator 2026-11-24): it is calculated at run time and may be cached in memory, but it is never written to the store. A row holds what was
//! written; one big tool result must not be stored again by every getter that shapes it (a chat message grew to 22 MB that way).
mod common;

use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};

fn tmpfile(tag: &str) -> String {
    let n = common::unique();
    std::env::temp_dir().join(format!("getter-{tag}-{}-{n}.sqlite3", std::process::id())).to_string_lossy().to_string()
}
fn seat(file: Option<&str>) -> Host {
    let mut o = Options::new(file.map(String::from));
    o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("gt", PACK).unwrap();
    h.rebuild();
    h
}
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }

const PACK: &str = "PACK gt 1
Thing.twice getter: this.n * 2
Thing.copy getter: this.payload
Thing.copy2 getter: [this.copy]
";

#[test]
fn a_getter_reads_its_value_but_the_row_holds_only_what_was_written() {
    let h = seat(None);
    h.born("Thing:t1", "Thing", props(json!({"n": 4, "payload": "big"})), "person");
    assert_eq!(h.get("Thing:t1", "twice"), json!(8), "the getter is computed");
    assert_eq!(h.get("Thing:t1", "copy2"), json!(["big"]), "and chained");
    let row = &h.rows()["Thing:t1"];
    let p = row["props"].as_object().unwrap();
    assert!(p.contains_key("n") && p.contains_key("payload"), "what was written is stored: {p:?}");
    for g in ["twice", "copy", "copy2"] { assert!(!p.contains_key(g), "the getter `{g}` is not stored: {p:?}"); }
    assert!(!row["writers"].as_object().unwrap().keys().any(|k| ["twice", "copy", "copy2"].contains(&k.as_str())), "nor its writer: {row}");
}

#[test]
fn a_getter_that_follows_a_change_is_still_right_and_a_restart_computes_it_again() {
    let f = tmpfile("restart");
    {
        let h = seat(Some(&f));
        h.born("Thing:t1", "Thing", props(json!({"n": 1, "payload": "a"})), "person");
        h.write("Thing:t1", "n", json!(5), "person");
        assert_eq!(h.get("Thing:t1", "twice"), json!(10));
    }
    let h = seat(Some(&f));
    h.load_stored_entities();
    assert_eq!(h.get("Thing:t1", "twice"), json!(10), "computed again from the restored inputs");
    assert!(h.rows()["Thing:t1"]["props"].get("twice").is_none(), "and still not stored");
}

const LAZY: &str = "PACK lz 1
Box.kind getter: 'box'
Box.go on-setter hit:
  WHEN this.kind == 'box':
    this.hit = 1
";

#[test]
fn a_setter_on_a_row_that_is_loaded_by_the_write_finds_the_getters_it_reads_computed() {
    let f = tmpfile("lazy");
    {
        let mut o = Options::new(Some(f.clone())); o.effects = false;
        let h = Host::open(o).unwrap();
        h.load_pack("lz", LAZY).unwrap(); h.rebuild();
        h.born("Box:a", "Box", props(json!({"n": 1})), "person");
    }
    let mut o = Options::new(Some(f.clone())); o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("lz", LAZY).unwrap(); h.rebuild();
    h.load_stored_entities();   // a lazy class: the row is only noted, the write below loads it
    assert!(h.rows()["Box:a"]["props"].get("kind").is_none(), "the getter's value is not in the row");
    h.write("Box:a", "go", json!(1), "person");
    assert_eq!(h.get("Box:a", "hit"), json!(1), "the setter read `this.kind`, which was computed before the setter ran");
}

const PULL: &str = "PACK pl 1
Req.go on-setter mk:
  ALWAYS:
    Msg:m = Msg(n: 3)
Msg.view getter: this.n * 2
Req.out getter: ('ok' if prop('Msg:m', 'view') != None else 'missing') if this.go != None else None
Req.out on-setter first:
  WHEN this.out != None and this.seen == None:
    this.seen = this.out
";

#[test]
fn a_getter_that_reads_a_getter_of_another_entity_is_computed_after_it_not_before() {
    let mut o = Options::new(None); o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("pl", PULL).unwrap(); h.rebuild();
    h.born("Req:r", "Req", props(json!({})), "person");
    h.write("Req:r", "go", json!(1), "person");
    assert_eq!(h.get("Req:r", "out"), json!("ok"), "converged");
    assert_eq!(h.get("Req:r", "seen"), json!("ok"), "and the first value anyone saw was the right one: the dependency was computed first");
}
