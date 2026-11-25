//! `${expr}`: the entity whose id is the value of `expr` (#1143, operator 2026-11-24). An id that arrives as data (`this.target`) is read like an entity literal: `${this.target}[this.prop]`
//! or `${this.target}.status`. The values and the errors are tests/c1ft/dyn_ent.c1ft; what stays here is the step budget, which c1ft cannot say. It is what `prop(id, name)` did, written as a value: `[name]` and `.name` after it read a property, so the builtins `prop` and `props` are gone.
use cliffvm::expr::{self, V};
use serde_json::{json, Value as J};
use std::collections::HashMap;

struct W(HashMap<(String, String), V>);
impl expr::World for W {
    fn attr(&self, eid: &str, attr: &str) -> V { self.0.get(&(eid.to_string(), attr.to_string())).cloned().unwrap_or_else(|| V::absent(format!("{eid}.{attr}"))) }
}
fn world() -> W {
    let mut m = HashMap::new();
    m.insert(("Command:x".to_string(), "status".to_string()), V::Str("ok".into()));
    m.insert(("Command:x".to_string(), "rc".to_string()), V::Int(0));
    m.insert(("Command:y".to_string(), "status".to_string()), V::Str("bad".into()));
    m.insert(("Command:x".to_string(), "ids".to_string()), expr::from_json(&json!(["x0", "x1", "x2"])));
    m.insert(("Command:y".to_string(), "ids".to_string()), expr::from_json(&json!(["y0", "y1"])));
    m.insert(("Command:y".to_string(), "d".to_string()), expr::from_json(&json!({"k": "one"})));
    W(m)
}

#[test]
fn it_is_one_step_however_many_ids() {
    let ids: J = J::Array((0..5000).map(|i| json!(format!("Command:{}", if i % 2 == 0 { "x" } else { "y" }))).collect());
    let n = expr::parse("${ids}['status']").unwrap();
    let env: HashMap<String, V> = [("ids".to_string(), expr::from_json(&ids))].into_iter().collect();
    expr::set_eval_budget(Some(8));
    let r = expr::eval(&n, &env, &world());
    expr::set_eval_budget(None);
    match r { Ok(V::List(l)) => assert_eq!(l.len(), 5000), other => panic!("under a budget of 8 steps: {other:?}") }
}


