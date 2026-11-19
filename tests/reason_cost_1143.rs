//! The cost of a reasoned null on the hot path (#1143): 3,000 getters each read a foreign property that has not arrived (`pending`), and a second pack's getters read a property
//! their entity never had (`absent`). The test prints the nanoseconds per getter; it asserts only that every value is a null with a reason (no wall-clock bound).
use cliffvm::fields::Fields;
use cliffvm::vm::{Core, Host as VmHost};
use serde_json::{json, Value as J};
use std::time::Instant;

struct H;
impl VmHost for H {
    fn resolve(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn resolve_exact(&self, s: &str) -> Option<String> { Some(s.to_string()) }
    fn dispatch(&self, _: &str, _: &str) -> Option<String> { None }
    fn vel(&self, _: &str, _: f64, _: f64) -> bool { false }
    fn align(&self, _: &str, _: f64) -> bool { false }
    fn live(&self, _: &str, _: bool) -> Option<String> { None }
}

fn run(pack: &str, n: usize) -> (f64, usize) {
    let mut f = Fields::goalgraph();
    f.set_pack("t", pack).unwrap();
    let mut c = Core::new();
    c.queue_writes = false;
    c.set_ns("p1");
    let mut seq = 0;
    let t = Instant::now();
    for i in 0..n {
        let eid = format!("Light:{i}");
        f.arrive(&mut c, eid, "Light".into(), json!({"x": 1}).as_object().unwrap().clone(), 1.0);
    }
    f.settle(&mut c, &H, 1.0, &mut seq);
    let ns = t.elapsed().as_nanos() as f64 / n as f64;
    let nulls = (0..n).filter(|i| c.docs[&format!("Light:{i}")].props.get("title").map_or(true, J::is_null)).count();
    (ns, nulls)
}

#[test]
fn the_cost_of_a_pending_and_of_an_absent_read_is_printed() {
    let n = 3000;
    for (name, pack) in [("pending", "PACK t 1\nLight.title getter: membrane::Setting:t.val\n"), ("absent", "PACK t 1\nLight.title getter: this.nope\n")] {
        let mut runs: Vec<f64> = (0..5).map(|_| { let (ns, nulls) = run(pack, n); assert_eq!(nulls, n, "every getter is a null"); ns }).collect();
        runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("REASON-COST {name}: median {:.0} ns per getter (min {:.0}, max {:.0}) over {n} getters", runs[2], runs[0], runs[4]);
    }
}
