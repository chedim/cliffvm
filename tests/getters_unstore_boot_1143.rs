//! A store that kept getter values (written before they became ephemeral, #1143, operator 2026-11-24) is cleaned at open: the properties a getter wrote, and their writer
//! stamps, are removed from the rows, once (`PRAGMA user_version`). What was written stays. A seat's 1.27 GB of chat rows held every tool result seven times that way.
mod common;

use cliffvm::host::{Host, Options, ROWS};
use rusqlite::Connection;
use serde_json::{json, Map, Value as J};

fn tmpfile(tag: &str) -> String {
    let n = common::unique();
    std::env::temp_dir().join(format!("getter-unstore-{tag}-{}-{n}.sqlite3", std::process::id())).to_string_lossy().to_string()
}
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }
fn open(f: &str) -> Host {
    let mut o = Options::new(Some(f.to_string()));
    o.effects = false;
    let h = Host::open(o).unwrap();
    h.load_pack("gt", "PACK gt 1\nThing.twice getter: this.n * 2\n").unwrap();
    h.rebuild();
    h
}
fn body(f: &str, k: &str) -> J {
    let db = Connection::open(f).unwrap();
    let b: String = db.query_row(&format!("SELECT body FROM {ROWS} WHERE k = ?"), [k], |r| r.get(0)).unwrap();
    serde_json::from_str(&b).unwrap()
}

#[test]
fn the_getter_values_an_old_store_kept_are_removed_at_open_and_what_was_written_stays() {
    let f = tmpfile("old");
    { let h = open(&f); h.born("Thing:t1", "Thing", props(json!({"n": 4})), "person"); }
    {   // the store as the old engine left it: a getter's value in the row, stamped as a getter's
        let db = Connection::open(&f).unwrap();
        let mut b = body(&f, "Thing:t1");
        b["props"]["twice"] = json!("x".repeat(1000));
        b["writers"]["twice"] = json!("trigger:field:getter:twice");
        b["props"]["blob"] = json!("y");
        b["writers"]["blob"] = json!("trigger:field:rebuild");
        db.execute(&format!("UPDATE {ROWS} SET body = ?1 WHERE k = 'Thing:t1'"), [b.to_string()]).unwrap();
        db.execute_batch("PRAGMA user_version = 0").unwrap();
    }
    let h = open(&f);
    let b = body(&f, "Thing:t1");
    assert!(b["props"].get("twice").is_none() && b["props"].get("blob").is_none(), "the getters' values are gone: {b}");
    assert!(b["writers"].get("twice").is_none() && b["writers"].get("blob").is_none(), "and their stamps: {b}");
    assert_eq!(b["props"]["n"], json!(4), "what was written stays: {b}");
    drop(h);
    let db = Connection::open(&f).unwrap();
    let v: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert!(v >= 1, "once: the store says it is clean");
}
