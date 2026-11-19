//! A null and an Error carry a VALUE as their reason (#1143): a text, or an entity. The engine's own reasons are entities of four classes (`Pending`, `Absent`, `Refused`,
//! `Collected`), so a pack asks `why(x).cls == 'Refused'` instead of matching the text. A pack's own `None('...')` stays a text; `None(<entity>)` and `Error(<entity>)` keep the entity.
use cliffvm::fields::{Fields, LinkVal};
use cliffvm::vm::{Core, Host as VmHost};
use serde_json::{json, Value as J};

struct H;
impl VmHost for H {
    fn resolve(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn resolve_exact(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn dispatch(&self, _: &str, _: &str) -> Option<String> { None }
    fn vel(&self, _: &str, _: f64, _: f64) -> bool { false }
    fn align(&self, _: &str, _: f64) -> bool { false }
    fn live(&self, _: &str, _: bool) -> Option<String> { None }
}

struct Vm { c: Core, f: Fields, seq: i64 }
impl Vm {
    fn new(pack: &str) -> Vm {
        let mut f = Fields::goalgraph();
        f.set_pack("t", pack).unwrap();
        let mut c = Core::new();
        c.queue_writes = false;
        c.set_ns("p1");
        Vm { c, f, seq: 0 }
    }
    fn born(&mut self, eid: &str, props: J) {
        let cls = cliffvm::vm::class_of_eid(eid);
        self.f.arrive(&mut self.c, eid.into(), cls, props.as_object().unwrap().clone(), 1.0);
        self.f.settle(&mut self.c, &H, 1.0, &mut self.seq);
    }
    fn prop(&self, eid: &str, p: &str) -> J { self.c.docs.get(eid).and_then(|e| e.props.get(p).cloned()).unwrap_or(J::Null) }
    fn val(&mut self, eid: &str, v: LinkVal, now: f64) { let mut s = self.seq; self.f.apply_val(&mut self.c, &H, eid, now, v, now, &mut s); self.seq = s; }
}
const SET: &str = "membrane::Setting:t";

#[test]
fn a_text_reason_stays_a_text() {
    let mut v = Vm::new("PACK t 1\nT.a getter: None('boom')\nT.r getter: why(this.a)\n");
    v.born("T:1", json!({}));
    assert_eq!(v.prop("T:1", "r"), json!("boom"));
}

#[test]
fn none_of_an_entity_keeps_the_entity_as_its_reason() {
    let mut v = Vm::new("PACK t 1\nT.a getter: None(this)\nT.k getter: why(this.a).cls\nT.n getter: why(this.a).eid\n");
    v.born("T:1", json!({}));
    assert_eq!(v.prop("T:1", "k"), json!("T"), "the reason is the entity T:1, not its id as text");
    assert_eq!(v.prop("T:1", "n"), json!("T:1"));
}

#[test]
fn an_error_of_an_entity_keeps_the_entity_even_after_it_is_stored() {
    let mut v = Vm::new("PACK t 1\nT.a getter: Error(this)\nT.k getter: why(this.a).cls\n");
    v.born("T:1", json!({}));
    assert_eq!(v.prop("T:1", "k"), json!("T"));
    assert_eq!(v.prop("T:1", "failed"), json!({"a": {"$ent": "T:1"}}), "stored: the entity is told apart from a text");
}

#[test]
fn a_property_nothing_wrote_is_an_absent_reason_with_its_text() {
    let mut v = Vm::new("PACK t 1\nT.k getter: why(this.nope).cls\nT.t getter: why(this.nope).text\nT.s getter: str(why(this.nope))\nT.i getter: why(this.nope) is Absent\nT.j getter: why(this.nope) is Refused\n");
    v.born("T:1", json!({"x": 1}));
    assert_eq!(v.prop("T:1", "k"), json!("Absent"));
    assert_eq!(v.prop("T:1", "t"), json!("absent: T:1.nope"));
    assert_eq!(v.prop("T:1", "s"), json!("absent: T:1.nope"), "str() of a reason entity is its text");
    assert_eq!(v.prop("T:1", "i"), json!(true), "`is` knows the class of an engine reason");
    assert_eq!(v.prop("T:1", "j"), json!(false));
}

#[test]
fn a_foreign_read_is_pending_then_absent_then_refused_as_classes() {
    let mut v = Vm::new("PACK t 1\nLight.c getter: why(membrane::Setting:t.val).cls\nLight.t getter: why(membrane::Setting:t.val).text\n");
    v.born("Light:a", json!({"x": 1}));
    assert_eq!(v.prop("Light:a", "c"), json!("Pending"));
    assert_eq!(v.prop("Light:a", "t"), json!(format!("pending: {SET}.val")));
    v.val(SET, LinkVal::Gone, 4.0);
    assert_eq!(v.prop("Light:a", "c"), json!("Absent"));
    v.val(SET, LinkVal::Refused("too large (70000 bytes)".into()), 5.0);
    assert_eq!(v.prop("Light:a", "c"), json!("Refused"));
    assert_eq!(v.prop("Light:a", "t"), json!("refused: too large (70000 bytes)"), "the peer's text is the text of the reason");
}

#[test]
fn a_read_of_a_collected_entity_is_a_collected_reason() {
    let mut v = Vm::new("PACK t 1\nT.c getter: why(Gone:g.x).cls\nT.t getter: why(Gone:g.x).text\n");
    v.c.note_collected("Gone:g");
    v.born("T:1", json!({}));
    assert_eq!(v.prop("T:1", "c"), json!("Collected"));
    assert_eq!(v.prop("T:1", "t"), json!("collected: nothing held it"));
}

#[test]
fn a_null_inside_a_list_keeps_its_reason_as_an_entity() {
    let mut v = Vm::new("PACK t 1\nT.l getter: [this.nope, 1]\nT.k getter: why(this.l[0]).cls\n");
    v.born("T:1", json!({}));
    assert_eq!(v.prop("T:1", "k"), json!("Absent"));
}

#[test]
fn the_chain_and_its_origin_name_the_class() {
    let mut v = Vm::new("PACK t 1\nT.a getter: this.b + 1\nT.b getter: this.nope\n");
    v.born("T:1", json!({}));
    let chain = v.f.why_chain(&v.c, "T:1", "a");
    let origin = chain.last().unwrap();
    assert_eq!(origin["reason"], json!("absent: T:1.nope"), "the text is what it was");
    assert_eq!(origin["reason_ent"], json!("Absent:T:1.nope"), "and the entity is named beside it");
}

#[test]
fn the_same_reasons_come_back_when_the_values_are_loaded_and_rebuilt() {
    let pack = "PACK t 1\nT.k getter: why(this.nope).cls\nT.e getter: Error(this)\nT.f getter: why(this.e).eid\nT.m getter: this.f\n";
    let mut a = Vm::new(pack);
    a.born("T:1", json!({"x": 1}));
    let stored = a.c.docs["T:1"].props.clone();
    assert_eq!(a.prop("T:1", "k"), json!("Absent"));
    // a restart loads what was stored (the reasons are not journalled: the getters give them again)
    let mut b = Vm::new(pack);
    b.c.load("T:1".to_string(), "T".to_string(), stored, None, 0.0);
    let mut s = 0; b.f.rebuild(&mut b.c, 1.0, &mut s);
    assert_eq!(b.prop("T:1", "k"), json!("Absent"));
    assert_eq!(b.prop("T:1", "f"), a.prop("T:1", "f"));
    assert_eq!(b.c.reason_of(&("T:1".into(), "e".into())), a.c.reason_of(&("T:1".into(), "e".into())));
}

#[test]
fn a_pack_that_treated_why_as_text_still_works_stored_compared_and_joined() {
    let mut v = Vm::new("PACK t 1\nT.w getter: why(this.nope)\nT.eq getter: why(this.nope) == 'absent: T:1.nope'\nT.j getter: 'because ' + why(this.nope)\nT.sw getter: startswith(why(this.nope), 'absent')\nT.l getter: len(why(this.nope))\n");
    v.born("T:1", json!({"x": 1}));
    assert_eq!(v.prop("T:1", "w"), json!("absent: T:1.nope"), "a stored reason is its text (an engine reason has no row to point at)");
    assert_eq!(v.prop("T:1", "eq"), json!(true));
    assert_eq!(v.prop("T:1", "j"), json!("because absent: T:1.nope"));
    assert_eq!(v.prop("T:1", "sw"), json!(true));
    assert_eq!(v.prop("T:1", "l"), json!("absent: T:1.nope".len()));
}
