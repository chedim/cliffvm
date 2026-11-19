//! The seat's driver and the slot reclaiming (graphlink docs/shared-memory.md, 4b): entities that are born and retired under a subscribed prefix become `Gone` cells (node and props), the driver
//! acknowledges the peer's removals and reuses its own acknowledged slots, and a cell that did not fit is kept and written once there is room. The test plays the peer with graphlink-shm
//! and graphlink-core, as any peer program would; the seat's table is too small to hold every entity at once.
#![cfg(all(unix, feature = "store"))]
mod common;

use cliffvm::host::{Host, Options};
use cliffvm::peers::Peers;
use graphlink_core::frame::{Op, Target, VERSION};
use graphlink_core::link::{Event, Link, Me};
use graphlink_core::transport::Transport;
use graphlink_shm::cells::{CellState, Cells, CellsReader, Reclaim};
use graphlink_shm::Shm;
use serde_json::{json, Map, Value as J};

const SPINS: u64 = 1000;
const DEPTH: usize = 8;

fn key(eid: &str, prop: &str) -> String { format!("{eid}\u{1}{prop}") }
fn props(v: i64) -> Map<String, J> { let mut m = Map::new(); m.insert("v".into(), json!(v)); m }

struct World { h: Host, peers: Peers, ctl: Shm, link: Link, sc: CellsReader, mine: Cells, rec: Reclaim, name: String }

impl World {
    fn start(slots: u64) -> World {
        let name = format!("r{}x{}", std::process::id() % 100000, common::unique().rsplit('-').next().unwrap());
        let mut o = Options::new(None); o.effects = false;
        let h = Host::open(o).unwrap();
        let decl = json!([{"name": name, "ring_bytes": 65536, "slots": slots, "key_cap": 128, "val_cap": 256, "spins": SPINS, "depth": DEPTH}]);
        let peers = Peers::from_fact("membrane", &decl, None, 0.0).unwrap();
        let ctl = Shm::attach(format!("/gl-{name}-ctl")).unwrap();
        let mine = Cells::create(format!("/gl-{name}-pc"), 8, 128, 256, 1).unwrap();   // the peer's own table: the seat reads it, and the peer's acknowledgements go in it
        let sc = CellsReader::attach(format!("/gl-{name}-sc")).unwrap();
        let link = Link::new(Me { ns: "tester".into(), build: "t".into(), epoch: 1.0, support: vec![VERSION], initiator: false });
        let mut w = World { h, peers, ctl, link, sc, mine, rec: Reclaim::new(), name };
        // the handshake: the seat's hello, ours, then the subscription to the prefix
        for _ in 0..50 { w.turn(); if w.link.is_open() { break; } }
        assert!(w.link.is_open(), "the link never opened");
        let f = w.link.send(vec![Op::Sub { target: Target::Prefix("membrane::Req:".into()), props: None, limit: None }]).unwrap();
        w.ctl.send(&f).unwrap();
        w.turn();
        w
    }
    /// one turn of the seat and of the peer's link
    fn turn(&mut self) {
        self.peers.pump(&self.h);
        let mut sent_hello = false;
        while let Ok(Some(f)) = self.ctl.recv() {
            let (events, reply) = self.link.receive(f);
            if let Some(r) = reply { self.ctl.send(&r).unwrap(); }
            if events.iter().any(|e| matches!(e, Event::Opened { .. })) { sent_hello = true; }
        }
        if sent_hello { let f = self.link.open(); self.ctl.send(&f).unwrap(); }
    }
    fn turns(&mut self, n: usize) { for _ in 0..n { self.turn(); } }
    /// the peer reads the seat's table and says what it has seen
    fn peer_reads(&mut self) -> Vec<(String, CellState)> {
        let seen = Reclaim::before(&self.sc);
        let rows = self.sc.scan("", SPINS, DEPTH).unwrap();
        self.rec.after(seen, &self.sc, &mut self.mine, SPINS, DEPTH).unwrap();
        rows.into_iter().map(|(k, c)| (k, c.state)).collect()
    }
    fn state(&self, eid: &str) -> Option<CellState> { self.sc.get(&key(eid, ""), SPINS, DEPTH).ok().flatten().map(|c| c.state) }
    fn wait(&mut self, what: &str, ok: impl Fn(&World) -> bool) { for _ in 0..200 { self.turn(); if ok(self) { return; } } panic!("{what}"); }
}
impl Drop for World { fn drop(&mut self) { for part in ["ctl", "sc", "pc"] { let _ = graphlink_shm::remove_region(&format!("/gl-{}-{part}", self.name)); } } }

#[test]
fn a_retired_entity_under_a_subscribed_prefix_becomes_gone_cells_node_and_props() {
    let mut w = World::start(16);
    w.h.born("membrane::Req:a", "Req", props(1), "t");
    w.wait("the entity's node cell is never published", |w| w.state("membrane::Req:a") == Some(CellState::Live));
    w.h.write("membrane::Req:a", "", J::Null, "t");   // retire
    w.wait("the retired entity's node cell never becomes Gone", |w| w.state("membrane::Req:a") == Some(CellState::Gone));
    let prop = w.sc.get(&key("membrane::Req:a", "v"), SPINS, DEPTH).unwrap().unwrap();
    assert_eq!(prop.state, CellState::Gone, "its props are Gone too, or their slots could never be reclaimed");
}

#[test]
fn entities_born_and_retired_round_after_round_pass_through_a_table_too_small_for_them_all() {
    // 8 slots; each round makes three entities of two cells each (six cells). Without reclaiming the second round does not fit.
    let mut w = World::start(8);
    for round in 0..5 {
        let ids: Vec<String> = (0..3).map(|i| format!("membrane::Req:r{round}e{i}")).collect();
        for (i, id) in ids.iter().enumerate() { w.h.born(id, "Req", props(i as i64), "t"); }
        w.wait(&format!("round {round}: the new entities never became cells"), |w| ids.iter().all(|id| w.state(id) == Some(CellState::Live)));
        for id in &ids { w.h.write(id, "", J::Null, "t"); }
        w.wait(&format!("round {round}: the retired entities never became Gone"), |w| ids.iter().all(|id| w.state(id) == Some(CellState::Gone)));
        let rows = w.peer_reads();                 // the peer has seen the removals: it says so
        assert!(rows.iter().filter(|(_, s)| *s == CellState::Gone).count() >= 3, "round {round}");
        w.turns(3);                                // the seat reads the acknowledgement and frees the slots
    }
}

#[test]
fn a_cell_that_does_not_fit_is_reported_once_however_many_turns_it_waits() {
    // the seat says "cannot publish <key> yet" for a waiting cell ONCE: the test runs the scenario in a child of itself, to read what it writes on stderr
    if std::env::var("CLIFFVM_TEST_FULL_TABLE_CHILD").is_ok() {
        let mut w = World::start(4);
        for id in ["membrane::Req:a", "membrane::Req:b"] { w.h.born(id, "Req", props(1), "t"); }
        w.wait("the first two entities never became cells", |w| ["membrane::Req:a", "membrane::Req:b"].iter().all(|id| w.state(id) == Some(CellState::Live)));
        w.h.born("membrane::Req:c", "Req", props(1), "t");   // no slot is left, and none is freed
        w.turns(300);
        return;
    }
    let me = std::env::current_exe().unwrap();
    let out = std::process::Command::new(me)
        .args(["--exact", "a_cell_that_does_not_fit_is_reported_once_however_many_turns_it_waits", "--nocapture", "--test-threads=1"])
        .env("CLIFFVM_TEST_FULL_TABLE_CHILD", "1").output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    let lines: Vec<&str> = err.lines().filter(|l| l.contains("cannot publish")).collect();
    let mut distinct = lines.clone(); distinct.sort(); distinct.dedup();
    assert!(!lines.is_empty(), "the waiting cells are not reported at all: {err}");
    assert_eq!(lines.len(), distinct.len(), "a waiting cell is reported again every turn: {} lines, {} distinct", lines.len(), distinct.len());
}
