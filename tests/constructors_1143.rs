//! Constructors (operator, 2026-11-12): `<entity> = Class(name: value, ...)`. The TARGET on the left is the entity's identity (no
//! `key` argument, no declared identity). It creates the entity at that eid by the creation path of a write (the class must be one
//! the pack owns or imports; refused otherwise, as a receipt, never a crash), sets the named arguments as its first properties, and
//! is idempotent: an entity that exists is not created again and a property that already has a value is not overwritten. A target
//! whose eid prefix is not the class (`membrane::Config:...` built as `Voice(...)`) is stamped `parents: ['cls:voice']`, the ISA
//! kind-mirror, so the engine mirrors it by kind.
//! What they do is tests/c1ft/constructors.c1ft; what stays here looks at the engine's own birth events and refusal lists, the print/parse round trip and the static type.
use cliffvm::fields::Fields;
use cliffvm::infer::infer_src;
use cliffvm::syntax::{parse, parse_full, print};
use cliffvm::vm::{Core, Host, Write};
use serde_json::{json, Value as J};

struct H;
impl Host for H {
    fn resolve(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn resolve_exact(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn dispatch(&self, _: &str, _: &str) -> Option<String> { None }
    fn vel(&self, _: &str, _: f64, _: f64) -> bool { false }
    fn align(&self, _: &str, _: f64) -> bool { false }
    fn live(&self, _: &str, _: bool) -> Option<String> { None }
}

/// The classes the pack owns: Thing, Reply (a getter each). Box:a has `props`; `pre` entities are loaded first; then Box.go is written.
const OWN: &str = "Thing.shown getter: this.eid\nReply.shown getter: this.eid\n";
fn run_with(pack: &str, props: J, pre: &[(&str, &str, J)]) -> (Core, Fields) {
    let mut f = Fields::goalgraph();
    f.set_pack("t", pack).unwrap_or_else(|e| panic!("{e}"));
    let mut c = Core::new();
    c.load("Box:a".into(), "Box".into(), props.as_object().unwrap().clone(), None, 0.0);
    for (e, cls, p) in pre { c.load(e.to_string(), cls.to_string(), p.as_object().unwrap().clone(), None, 0.0); }
    let mut seq = 0;
    f.rebuild(&mut c, 1.0, &mut seq);
    for _ in 0..2 {   // twice: the second write of the same value changes nothing, a second `go` is a fresh trigger
        seq += 1;
        let w = Write { eid: "Box:a".into(), prop: "go".into(), new: json!(seq), old: J::Null, writer: "person".into(), chain: json!({}), seq };
        c.hear(w.clone(), None, 2.0);
        f.react(&mut c, &H, w, 2.0, &mut seq);
    }
    (c, f)
}
fn run(body: &str, props: J) -> (Core, Fields) { run_with(&format!("PACK t 1\n{OWN}Box.go on-setter t:\n  ALWAYS:\n{body}"), props, &[]) }
fn born(f: &Fields) -> Vec<String> { f.out.iter().filter_map(|o| o.get("born").map(|_| o["eid"].as_str().unwrap().to_string())).collect() }
fn p<'a>(c: &'a Core, e: &str) -> &'a serde_json::Map<String, J> { &c.docs.get(e).unwrap_or_else(|| panic!("no entity {e}: {:?}", c.docs.keys().collect::<Vec<_>>())).props }

#[test]
fn a_constructor_is_born_once_though_the_setter_runs_twice() {
    let (c, f) = run("    Thing:t1 = Thing(kind: 'in', n: this.n + 1)\n", json!({"n": 4}));
    assert_eq!(c.docs["Thing:t1"].cls, "Thing");
    assert_eq!(born(&f), ["Thing:t1"], "born once, though the setter ran twice");
}

#[test]
fn an_entity_that_exists_is_not_born_again() {
    let pack = format!("PACK t 1\n{OWN}Box.go on-setter t:\n  ALWAYS:\n    Thing:t1 = Thing(kind: 'new', n: 1, note: None)\n");
    let pre = [("Thing:t1", "Thing", json!({"kind": "old", "n": null}))];
    let (c, f) = run_with(&pack, json!({}), &pre);
    assert!(born(&f).is_empty(), "an entity that exists is not created again");
}

#[test]
fn a_foreign_entity_that_exists_is_not_born_again_and_keeps_its_class_list() {
    let pack = format!("PACK t 1\n{OWN}Box.go on-setter t:\n  ALWAYS:\n    membrane::Config:voice:in:z = Thing(kind: 'in', person: 'Z')\n");
    let pre = [("membrane::Config:voice:in:z", "Config", json!({"person": "Zed", "parents": ["cls:thing"]}))];
    let (c, f) = run_with(&pack, json!({}), &pre);
    assert_eq!(p(&c, "membrane::Config:voice:in:z")["parents"], json!(["cls:thing"]), "loaded as it was stored: the constructor added no class");
    assert!(born(&f).is_empty());
}

#[test]
fn a_class_the_pack_does_not_own_or_import_is_refused_with_a_reason() {
    let (c, f) = run("    Thing:t1 = Nope(kind: 'in')\n    Thing:t2 = Goal(kind: 'in')\n    Thing:t3 = Thing(kind: 'ok')\n", json!({}));
    assert!(!c.docs.contains_key("Thing:t1"), "no pack owns Nope");
    assert!(!c.docs.contains_key("Thing:t2"), "goalgraph owns Goal and this pack does not import it");
    assert_eq!(p(&c, "Thing:t3")["kind"], json!("ok"), "the line below still runs");
    let why: Vec<String> = f.refusals.iter().map(|r| r["reason"].as_str().unwrap().to_string()).collect();
    assert!(why.iter().any(|w| w.starts_with("error:") && w.contains("Nope")), "{why:?}");
    assert!(why.iter().any(|w| w.contains("Goal") && w.contains("IMPORT")), "{why:?}");
}

#[test]
fn a_null_or_bad_key_creates_nothing_and_says_why() {
    let (c, f) = run("    Thing:k[this.missing] = Thing(kind: 'x')\n    Thing:k[this.f] = Thing(kind: 'x')\n", json!({"f": 1.5}));
    assert!(!c.docs.keys().any(|k| k.starts_with("Thing:k")));
    assert!(f.refusals.iter().any(|r| r["reason"].as_str().unwrap().starts_with("error:")), "{:?}", f.refusals);
}

#[test]
fn print_and_parse_round_trip_a_constructor() {
    let src = format!("PACK t 1\n{OWN}Box.go on-setter t:\n  ALWAYS:\n    membrane::Chat:message[this.id]:response = Reply(text: this.t, n: 1)\n");
    let a = parse(&src).unwrap();
    assert_eq!(a, parse(&print(&a)).unwrap());
    assert!(print(&a).contains("membrane::Chat:message[this.id]:response = Reply(text: this.t, n: 1)"), "{}", print(&a));
}

#[test]
fn a_constructor_is_typed_as_its_class() {
    assert_eq!(infer_src("Voice(kind: 'in')", &[]).unwrap().0, "Entity(Voice)");
    assert_eq!(infer_src("Voice(kind: 1 + 'a')", &[]).unwrap().1.len(), 1, "its arguments are typed for findings");
}

