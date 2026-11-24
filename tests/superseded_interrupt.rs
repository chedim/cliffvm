//! A query a getter stopped asking (superseded by another question) is interrupted; asking it AGAIN before its thread has ended must start it afresh, and the
//! interrupted run's `Err("sql: interrupted ...")` must never be stored as the answer of the new ask (the getter would keep that error until a dependency changes).
use cliffvm::store::{Poll, SqlExec, Store};
use serde_json::{json, Value as J};
use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct S { interrupted: HashSet<u64>, gate: bool, release: bool }

/// `slow` blocks until it is interrupted AND the test opened the gate (the interrupt lands late, as it does on a loaded runner: the thread has not ended when the
/// getter asks again), or until the test releases it; any other query answers at once.
struct Fake { st: Mutex<S>, cv: Condvar }

impl SqlExec for Fake {
    fn run(&self, token: u64, sql: &str, _args: &[J]) -> Result<Vec<J>, String> {
        if sql != "slow" { return Ok(vec![json!({"q": sql})]); }
        let mut g = self.st.lock().unwrap();
        loop {
            if g.interrupted.contains(&token) && g.gate { return Err("sql: interrupted (the getter asked another question)".into()); }
            if g.release && !g.interrupted.contains(&token) { return Ok(vec![json!({"q": "slow"})]); }
            g = self.cv.wait_timeout(g, Duration::from_millis(20)).unwrap().0;
        }
    }
    fn interrupt(&self, token: u64) { self.st.lock().unwrap().interrupted.insert(token); self.cv.notify_all(); }
}

#[test]
fn a_query_asked_again_after_it_was_superseded_never_takes_the_interrupted_error() {
    let fake = Arc::new(Fake { st: Mutex::new(S::default()), cv: Condvar::new() });
    let store = Store::with_exec("", fake.clone());
    assert!(matches!(store.request("g", "slow", &[]), Poll::Pending));       // A runs
    // B supersedes A: A is interrupted, its thread is still running
    let mut b = store.request("g", "fast", &[]);
    let t = Instant::now();
    while matches!(b, Poll::Pending) && t.elapsed() < Duration::from_secs(10) { std::thread::sleep(Duration::from_millis(10)); b = store.request("g", "fast", &[]); }
    assert!(matches!(b, Poll::Ready(Ok(_))), "B answers");
    // the getter asks A again before A's thread has ended
    assert!(matches!(store.request("g", "slow", &[]), Poll::Pending));
    // now the interrupt lands: the old run ends with its error
    { let mut g = fake.st.lock().unwrap(); g.gate = true; }
    fake.cv.notify_all();
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(600) {
        match store.request("g", "slow", &[]) {
            Poll::Ready(Err(e)) => panic!("the interrupted run's error was stored as the answer: {e}"),
            Poll::Ready(Ok(_)) => panic!("the new run was never released"),
            Poll::Pending => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // the fresh run of A answers once it is released
    { let mut g = fake.st.lock().unwrap(); g.release = true; }
    fake.cv.notify_all();
    let t = Instant::now();
    loop {
        match store.request("g", "slow", &[]) {
            Poll::Ready(r) => { assert_eq!(r, Ok(vec![json!({"q": "slow"})])); break; }
            Poll::Pending => {}
        }
        assert!(t.elapsed() < Duration::from_secs(10), "the fresh run never answered");
        std::thread::sleep(Duration::from_millis(10));
    }
}
