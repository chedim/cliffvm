//! Static type inference over c1f getters (#1143, operator's design 2026-11-12): a REPORT, it enforces nothing.
//! A getter's type follows from its expression and the types of what it reads. Plain properties written from outside are
//! untyped edges (Any). Nulls propagate like SQL's; `x or d` strips the null; `Int(x)` is a checked cast (value or a null).
use cliffvm::infer::{infer_pack, infer_packs, infer_src};

fn ty(src: &str, vars: &[(&str, &str)]) -> String { infer_src(src, vars).unwrap().0 }
fn found(src: &str, vars: &[(&str, &str)]) -> Vec<String> { infer_src(src, vars).unwrap().1 }

#[test]
fn certain_errors_are_findings() {
    assert!(found("'a' + 1", &[])[0].contains("cannot apply +"), "{:?}", found("'a' + 1", &[]));
    assert_eq!(found("t + 1", &[("t", "Text")]).len(), 1);
    assert_eq!(found("1 + t", &[("t", "Text?")]).len(), 1);
    assert!(found("'a' < 1", &[])[0].contains("cannot compare"));
    assert!(found("len(3)", &[])[0].contains("len"));
    assert!(found("3[0]", &[])[0].contains("cannot index"));
    assert!(found("t[0]", &[("t", "Text")])[0].contains("cannot index"));
    assert!(found("d.x", &[("d", "Dict")])[0].contains("attribute"));
    assert!(found("lower(l)", &[("l", "List(Int)")])[0].contains("lower"));
    assert!(found("len(1, 2)", &[])[0].contains("argument"));
    assert!(found("[x for x in 3]", &[])[0].contains("iterate"));
}

#[test]
fn what_might_be_fine_is_not_a_finding() {
    for (src, vars) in [("x + 1", vec![("x", "Any")]), ("x + 1", vec![("x", "Int|Text")]), ("x < 1", vec![("x", "Any")]),
        ("len(x)", vec![("x", "Text?")]), ("x[0]", vec![("x", "Any")]), ("'a' + x", vec![("x", "Any")]), ("x + 1", vec![("x", "Int?")]),
        ("None + 1", vec![]), ("1 == 'a'", vec![]), ("lower(x)", vec![("x", "Int")])] {
        assert!(found(src, &vars).is_empty(), "{src}: {:?}", found(src, &vars));
    }
}

// ---- packs --------------------------------------------------------------------------------------------------------------

#[test]
fn a_getter_reads_other_getters_in_any_order() {
    let r = infer_pack("PACK p 1\nA.b getter: this.a + 1\nA.a getter: 1\nA.c getter: this.b / 2\nA.d getter: this.zzz\n").unwrap();
    assert_eq!(r.getters["A.a"], "Int");
    assert_eq!(r.getters["A.b"], "Int");
    assert_eq!(r.getters["A.c"], "Float");
    assert_eq!(r.getters["A.d"], "Any");     // a plain property written from outside is an untyped edge
}

#[test]
fn a_cycle_is_any_and_a_finding() {
    let r = infer_pack("PACK p 1\nA.a getter: this.b + 1\nA.b getter: this.a + 1\nA.c getter: this.a + 1\n").unwrap();
    assert_eq!(r.getters["A.a"], "Any");
    assert_eq!(r.getters["A.b"], "Any");
    assert!(r.findings.iter().any(|f| f.contains("cycle")), "{:?}", r.findings);
    // a getter that reads its own name reads the written value (the fallback arm `X.f getter: this.f`): an edge, not a cycle
    let own = infer_pack("PACK p 1\nA.a getter WHEN this.x == 1: 1\nA.a getter: this.a\n").unwrap();
    assert_eq!(own.getters["A.a"], "Any");
    assert!(own.findings.is_empty(), "{:?}", own.findings);
}

#[test]
fn a_getter_arm_without_a_default_is_nullable() {
    let r = infer_pack("PACK p 1\nA.a getter:\n  WHEN this.x == 1: 'one'\n  ELSE WHEN this.x == 2: 'two'\nA.b getter:\n  WHEN this.x == 1: 1\n  ELSE: 2\n").unwrap();
    assert_eq!(r.getters["A.a"], "Text?");
    assert_eq!(r.getters["A.b"], "Int");
}

#[test]
fn entities_from_instances_carry_their_class() {
    let r = infer_pack("PACK p 1\nBoot.n getter: 3\nA.xs getter: [b.n for b in instances('Boot')]\nA.ys getter: [b.q for b in instances('Boot')]\nA.z getter: ([b.n for b in instances('Boot')] or [0])[0]\n").unwrap();
    assert_eq!(r.getters["A.xs"], "List(Int)");
    assert_eq!(r.getters["A.ys"], "List(Any)");
    assert_eq!(r.getters["A.z"], "Int?");
}

#[test]
fn setter_right_hand_sides_are_collected_as_writes() {
    let r = infer_pack("PACK p 1\nA.go on-setter s1:\n  WHEN this.go == 1:\n    this.p = 'x'\n  ELSE:\n    this.p = 1\n    ('Goal:' + this.go).status = 'open'\n    this.go = None\nA.p getter: 3\n").unwrap();
    assert_eq!(r.writes["A.p"], "Int|Text (getter: Int)");
    assert_eq!(r.writes["Goal.status"], "Text");
    assert_eq!(r.writes["A.go"], "None");
}

#[test]
fn a_setter_loop_binds_its_variable_and_errors_in_setters_are_found() {
    let r = infer_pack("PACK p 1\nBoot.n getter: 3\nA.go on-setter s1:\n  FOR b IN instances('Boot'):\n    this.m = b.n\n  this.k = 'a' + 1\n").unwrap();
    assert_eq!(r.writes["A.m"], "Int");
    assert!(r.findings.iter().any(|f| f.contains("A.go") && f.contains("cannot apply +")), "{:?}", r.findings);
}

#[test]
fn packs_resolve_each_others_classes() {
    let r = infer_packs(&["PACK a 1\nX.n getter: 1\n", "PACK b 1\nIMPORT a 1\nX.m getter: this.n + 1\nY.k getter: [x.n for x in instances('X')]\n"]).unwrap();
    assert_eq!(r[1].0, "b");
    assert_eq!(r[1].1.getters["X.m"], "Int");
    assert_eq!(r[1].1.getters["Y.k"], "List(Int)");
    assert!(r[0].1.findings.is_empty());
}

#[test]
fn findings_in_a_getter_name_the_getter() {
    let r = infer_pack("PACK p 1\nA.a getter: 'x' + 1\nA.b getter: 2\n").unwrap();
    assert!(r.findings.iter().any(|f| f.starts_with("A.a: ")), "{:?}", r.findings);
    assert_eq!(r.getters["A.a"], "Any");
}
