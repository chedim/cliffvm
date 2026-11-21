//! WHO COMPETES, AND NEVER IN SILENCE (#1143, operator 2026-11-21): the first-write-wins rule of layers (docs/c1f-layers.md) is between SIBLINGS, the packs that stack on one class. A write
//! from a pack that is not one of that class's layers (the door's `entity_write` is a setter of the `door_tools` pack) is an INPUT to the class, like a person's write: it claims nothing, and the
//! class's own setters answer it normally (a setter may rewrite the property that fired it). When siblings really do compete and the skipped write is a setter rewriting its OWN trigger,
//! the skip is not quiet: the entity carries a `refused` receipt that names the setter and the pack that won. Found on a seat: Memo's `fix` setter rewrote `a` and was skipped for hours because
//! the door pack had written `a` first.
mod common;
use cliffvm::host::{Host, Options};
use serde_json::{json, Map, Value as J};

fn open() -> Host { let mut o = Options::new(None); o.effects = false; Host::open(o).unwrap() }
fn props(v: J) -> Map<String, J> { v.as_object().unwrap().clone() }

// like the door's `entity_write`: the target and the property are COMPUTED, so no load-time check can tell which class is written (a literal `Probe2:y.a` is refused at once: pack trig does not import probe2)
const TRIG: &str = "PACK trig 1\nTrig.kind getter: 'trig'\nTrig.go on-setter push:\n  ALWAYS:\n    (this.target).[this.tprop] = 'open'\n";
const FIX: &str = "PACK probe2 1\nProbe2.kind getter: 'probe2'\nProbe2.a on-setter fix:\n  WHEN this.a == 'open':\n    this.a = 'owed'\n";
// ALWAYS conditions: both setters fire on the one write and both rewrite `a` (a WHEN on `a` would see the upper layer's rewrite and the lower setter would not even run)
const LOWER: &str = "PACK lower 1\nProbe5.kind getter: 'p5'\nProbe5.a on-setter lowfix:\n  ALWAYS:\n    this.a = 'low'\n";
const UPPER: &str = "PACK upper 1\nIMPORT lower 1\nProbe5.a on-setter upfix:\n  ALWAYS:\n    this.a = 'up'\n";

#[test]
fn a_write_from_a_pack_that_is_not_a_layer_of_the_class_is_an_input_and_the_classs_setter_may_rewrite_it() {
    let h = open();
    // `trig` is loaded as the FIRST app (the root of trust), as `door_tools` is: a privileged pack may write any class, which is how the door's `entity_write` writes the entities of packs it does not import
    let dir = std::env::temp_dir().join(format!("loudguard-{}-{}", std::process::id(), common::unique()));
    std::fs::create_dir_all(&dir).unwrap();
    let trig = dir.join("trig.c1f");
    std::fs::write(&trig, TRIG).unwrap();
    h.load_app(&trig, true).unwrap();
    h.load_pack("probe2", FIX).unwrap();
    h.rebuild();
    h.born("Probe2:y", "Probe2", props(json!({"a": "idle"})), "person");
    h.born("Trig:t", "Trig", props(json!({"target": "Probe2:y", "tprop": "a"})), "person");
    h.write("Trig:t", "go", json!(1), "person");
    assert_eq!(h.get("Probe2:y", "a"), json!("owed"), "the `trig` pack wrote `a` (an input); `fix`, a layer of Probe2, rewrote it");
    assert_eq!(h.get("Probe2:y", "refused"), J::Null, "nothing was refused");
}

#[test]
fn when_siblings_compete_a_skipped_rewrite_of_the_setters_own_trigger_leaves_a_receipt_naming_both() {
    let h = open();
    h.load_pack("lower", LOWER).unwrap();
    h.load_pack("upper", UPPER).unwrap();
    h.rebuild();
    h.born("Probe5:y", "Probe5", props(json!({"a": "idle"})), "person");
    h.write("Probe5:y", "a", json!("open"), "person");
    let receipt = h.get("Probe5:y", "refused").as_str().unwrap_or("").to_string();
    assert!(receipt.contains("lowfix") && receipt.contains("upper"), "a=`{}`: the receipt names the skipped setter and the pack that won: {receipt:?}", h.get("Probe5:y", "a"));
}
