//! `Error` in the field (#1143, docs/c1f-error.md section (c), operator decisions 2026-11-13): a derived Error is stored as a plain null
//! with its kind in `Core.why` (so the old routes see null); a getter reading an Error returns the same Error unchanged; a WHEN over an
//! Error is that Error (it does not fall through to the next arm); a setter that writes an Error is refused with a `refused` receipt.
use cliffvm::fields::Fields;
use cliffvm::vm::{Core, Host as VmHost};
use serde_json::json;

struct H;
impl VmHost for H {
    fn resolve(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn resolve_exact(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn dispatch(&self, _: &str, _: &str) -> Option<String> { None }
    fn vel(&self, _: &str, _: f64, _: f64) -> bool { false }
    fn align(&self, _: &str, _: f64) -> bool { false }
    fn live(&self, _: &str, _: bool) -> Option<String> { None }
}

fn world(packs: &[(&str, &str)]) -> (Core, Fields) {
    let mut f = Fields::goalgraph();
    for (n, t) in packs { f.set_pack(n, t).unwrap_or_else(|e| panic!("{n}: {e}")); }
    let mut c = Core::new();
    c.load("Light:a".into(), "Light".into(), json!({"level": 0}).as_object().unwrap().clone(), None, 0.0);
    let mut seq = 0;
    f.rebuild(&mut c, 1.0, &mut seq);
    (c, f)
}

#[test]
fn the_failure_lives_in_the_entitys_failed_and_a_null_keeps_its_note_in_core_why() {
    let (c, _f) = world(&[("p", "PACK p 1\nLight.bad getter: Error('x broke')\nLight.nul getter: None('x absent')\n")]);
    let (bad, nul) = (("Light:a".to_string(), "bad".to_string()), ("Light:a".to_string(), "nul".to_string()));
    assert_eq!(c.docs["Light:a"].failure("bad").map(|r| r.text()).as_deref(), Some("x broke"));
    assert_eq!(c.docs["Light:a"].failure("nul"), None);
    assert_eq!(c.why.get(&bad), None, "a failure's reason is not in the side map any more");
    assert_eq!(c.why.get(&nul).map(|r| r.text()).as_deref(), Some("x absent"));
}
