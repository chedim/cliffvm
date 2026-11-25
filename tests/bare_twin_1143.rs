//! A bare id falls back to the seat's own namespace (#1143, operator 2026-11-25): `Thing:t1` reads `membrane::Thing:t1` when no entity has the bare id.
//! The seat lists its entities under `membrane::`; a model that writes the id without the prefix reads the same entity. The bare id wins when both exist,
//! and an id found in neither is the same reasoned null as before.
mod common;

use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};

fn seat() -> Host {
    let mut o = Options::new(None);
    o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("bt", PACK).unwrap();
    h.rebuild();
    h
}
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }

const PACK: &str = "PACK bt 1
Reader.one getter: ${this.target}['n']
Reader.lit getter: ${'Thing:t1'}['n']
Reader.kind getter: ${this.target}['cls']
Reader.shown getter: meta(this.target)
Reader.gone getter: ${'Thing:nope'}['n'] == None and (why(${'Thing:nope'}['n']) is Absent)
Reader.many getter: ${this.targets}['n']
";

#[test]
fn a_bare_id_reads_the_entity_the_seat_holds_under_its_own_namespace() {
    let h = seat();
    h.born("membrane::Thing:t1", "Thing", props(json!({"n": 4})), "person");
    h.born("Reader:r1", "Reader", props(json!({"target": "Thing:t1"})), "person");
    assert_eq!(h.get("Reader:r1", "one"), json!(4));
    assert_eq!(h.get("Reader:r1", "lit"), json!(4), "the same in a literal");
    assert_eq!(h.get("Reader:r1", "kind"), json!("Thing"));
    let m = h.get("Reader:r1", "shown");
    assert_eq!(m["properties"], json!(["n"]), "meta of the bare id names the twin's properties: {m}");
}

#[test]
fn a_list_of_bare_ids_reads_each_one() {
    let h = seat();
    h.born("membrane::Thing:a", "Thing", props(json!({"n": 1})), "person");
    h.born("Thing:b", "Thing", props(json!({"n": 2})), "person");
    h.born("Reader:r2", "Reader", props(json!({"targets": ["Thing:a", "Thing:b"]})), "person");
    assert_eq!(h.get("Reader:r2", "many"), json!([1, 2]), "one under the prefix, one bare");
}

#[test]
fn the_bare_entity_wins_when_both_exist() {
    let h = seat();
    h.born("membrane::Thing:t1", "Thing", props(json!({"n": 4})), "person");
    h.born("Thing:t1", "Thing", props(json!({"n": 9})), "person");
    h.born("Reader:r3", "Reader", props(json!({"target": "Thing:t1"})), "person");
    assert_eq!(h.get("Reader:r3", "one"), json!(9));
}

#[test]
fn an_id_found_in_neither_place_is_absent_as_before() {
    let h = seat();
    h.born("Reader:r4", "Reader", props(json!({})), "person");
    assert_eq!(h.get("Reader:r4", "gone"), json!(true));
}

#[test]
fn a_getter_reading_the_bare_id_follows_the_twin_when_it_changes() {
    let h = seat();
    h.born("membrane::Thing:t1", "Thing", props(json!({"n": 4})), "person");
    h.born("Reader:r5", "Reader", props(json!({"target": "Thing:t1"})), "person");
    assert_eq!(h.get("Reader:r5", "one"), json!(4));
    h.write("membrane::Thing:t1", "n", json!(5), "person");
    assert_eq!(h.get("Reader:r5", "one"), json!(5), "the twin changed: the reader is dirty");
}

#[test]
fn a_getter_that_read_nothing_finds_the_twin_when_it_is_born_later() {
    let h = seat();
    h.born("Reader:r6", "Reader", props(json!({"target": "Thing:late"})), "person");
    assert!(h.get("Reader:r6", "one").is_null());
    h.born("membrane::Thing:late", "Thing", props(json!({"n": 7})), "person");
    assert_eq!(h.get("Reader:r6", "one"), json!(7), "the twin is born: the reader is dirty");
}

#[test]
fn another_namespace_is_never_mapped() {
    let h = seat();
    h.born("membrane::Thing:t1", "Thing", props(json!({"n": 4})), "person");
    h.born("Reader:r7", "Reader", props(json!({"target": "other::Thing:t1"})), "person");
    assert!(h.get("Reader:r7", "one").is_null(), "a foreign id is not this seat's");
}
