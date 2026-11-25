//! Null reasons and the mention cascade (#1143, the model's live findings): a missing property is a null that says `absent`
//! (naming the entity and the property); a reasoned null inside a list or a dict keeps its reason. What they say is tests/c1ft/semantics.c1ft; what stays here
//! is the restart that reopens the store with another pack header.
mod common;

use cliffvm::host::{Host, Options};
use serde_json::{json, Value as J};

// ---- a class a pack counts with `instances` is declared RESIDENT by the pack that owns it ----
const WATCH: &str = "Keep.line getter: 'k ' + str(this.title)\n\
Watch.keeps_now getter: len([k for k in instances('Keep') if k.done != True])\n\
Watch.snap on-setter snapshot:\n  ALWAYS:\n    this.last_keeps = this.keeps_now\n\
Watch.alert getter: 'ALERT' if this.last_keeps != None and this.keeps_now < this.last_keeps else ''\n";

fn store_file(tag: &str) -> String {
    std::env::temp_dir().join(format!("sem-{tag}-{}-{}.sqlite3", std::process::id(), common::unique())).to_string_lossy().to_string()
}
fn open(file: &str) -> Host { let mut o = Options::new(Some(file.to_string())); o.effects = false; Host::open(o).unwrap() }

#[test]
fn a_class_the_owner_declares_resident_is_counted_whole_after_a_restart_where_a_lazy_one_is_not() {
    let file = store_file("resident");
    {
        let h = open(&file);
        h.load_pack("plan", &format!("PACK plan 1\n{WATCH}")).unwrap();
        h.rebuild();
        for i in 0..6 { h.born(&format!("Keep:k{i}"), "Keep", json!({"title": format!("t{i}")}).as_object().unwrap().clone(), "person"); }
        h.born("Watch:main", "Watch", Default::default(), "person");
        h.pump();
    }
    let count = |header: &str| {
        let h = open(&file);
        h.set_warm_rows(0);
        h.load_pack("plan", &format!("PACK plan 1\n{header}{WATCH}")).unwrap();
        h.load_stored_entities();
        h.rebuild();
        h.pump();
        h.get("Watch:main", "keeps_now")
    };
    let lazy = count("");
    assert!(lazy.as_i64().unwrap_or(0) < 6, "a lazy class lists only what is loaded: {lazy}");
    assert_eq!(count("RESIDENT Keep\n"), json!(6), "the owner declares it RESIDENT: every stored Keep is counted");
    let _ = std::fs::remove_file(&file);
}
