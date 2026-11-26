//! PRIVILEGE (docs/c1f-layers.md, "Privilege"): exemption from rule B (a pack writes only classes it owns or imports) ONLY. Rule A stays
//! (only the owner defines getters and setters on a class). The grant is a write, `Pack:<name>.privileged = true`, by a privileged writer;
//! the host records `granted_by` from the writer; the boot pack is the root of trust. What the grants, the refusals and the replay do is tests/c1ft/privilege.c1ft;
//! what stays here is rule A with a pack granted before it loads, and the restart that reads a stored grant back.
mod common;

use cliffvm::expr;
use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

thread_local! { static NOW: Cell<u64> = const { Cell::new(0) }; }
fn fake() -> u64 { NOW.with(|n| n.get()) }

fn ticking() -> Box<dyn Fn() -> f64 + Send + Sync> {
    let n = Arc::new(AtomicU64::new(0));
    Box::new(move || 1000.0 + n.fetch_add(1, Ordering::SeqCst) as f64)
}
/// `own` owns Cls
const OWN: &str = "PACK own 1\nCls.tag getter: 'tag'\n";
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }
fn host() -> Host {
    NOW.with(|n| n.set(0));
    expr::set_clock(Some(fake));
    let h = Host::open({ let mut o = Options::new(None); o.clock = ticking(); o.effects = false; o }).unwrap();
    h.rebuild();
    h
}

#[test]
fn a_privileged_pack_still_cannot_define_getters_or_setters_on_a_class_it_does_not_own_rule_a() {
    let h = host();
    h.load_pack("own", OWN).unwrap();
    h.born("Pack:pp", "Pack", props(json!({"privileged": true})), "person");
    let e = h.load_pack("pp", "PACK pp 1\nCls.extra getter: 'x'\n").unwrap_err();
    assert!(e.contains("belongs to pack own"), "{e}");
    let e = h.load_pack("pp", "PACK pp 1\nCls.n on-setter s:\n  ALWAYS: this.m = 1\n").unwrap_err();
    assert!(e.contains("belongs to pack own"), "{e}");
    h.load_pack("pp", "PACK pp 1\nIMPORT own 1\nCls.extra getter: 'x'\n").unwrap();   // with the import it is the old rule: allowed
}

#[test]
fn a_grant_for_a_file_loaded_pack_does_not_remove_it_and_survives_a_restart() {
    let file = std::env::temp_dir().join(format!("priv-{}-{}.sqlite3", std::process::id(), common::unique())).to_string_lossy().to_string();
    let open = || { let mut o = Options::new(Some(file.clone())); o.clock = ticking(); o.effects = false; Host::open(o).unwrap() };
    let t = "PACK filed 1\nBox.label getter: 'f' + str(this.w)\n";
    NOW.with(|n| n.set(0));
    expr::set_clock(Some(fake));
    {
        let h = open();
        h.load_pack("filed", t).unwrap();
        h.rebuild();
        h.born("Box:a", "Box", props(json!({"w": 1})), "person");
        h.born("Pack:filed", "Pack", props(json!({"privileged": true})), "person");   // no text
        assert!(h.pack_privileged("filed"));
        h.write("Box:a", "w", json!(2), "person");
        assert_eq!(h.get("Box:a", "label"), json!("f2"), "the pack still runs");
        assert_eq!(h.get("Pack:filed", "loaded"), json!(true));
    }
    let h = open();                                              // the seat's start: the folder's packs, then the stored Pack rows
    h.load_pack("filed", t).unwrap();
    h.load_stored_packs();
    h.rebuild();
    assert!(h.pack_privileged("filed"), "the stored grant is read back");
    h.born("Box:a", "Box", props(json!({"w": 3})), "person");      // (the engine starts empty: rows are not loaded back into it)
    assert_eq!(h.get("Box:a", "label"), json!("f3"), "a Pack entity with no text does not remove a file-loaded pack");
    // an explicit empty text does
    h.write("Pack:filed", "text", json!(""), "person");
    assert_eq!(h.get("Pack:filed", "loaded"), json!(false));
    let _ = std::fs::remove_file(&file);
}

