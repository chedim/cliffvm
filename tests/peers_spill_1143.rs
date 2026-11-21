//! The seat's driver and a value larger than a cell (graphlink docs/shared-memory.md, 4c): a value over the peer table's value capacity is published whole, as the cells of its pieces, and the peer
//! reads it back whole; it is never a `Refused` cell that names a size. The pieces go back to the table when the value is replaced or its entity is retired. The test plays the peer with
//! graphlink-shm and graphlink-core, as any peer program would.
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
const VAL_CAP: u32 = 1024;

fn key(eid: &str, prop: &str) -> String { format!("{eid}\u{1}{prop}") }
fn props(text: &str) -> Map<String, J> { let mut m = Map::new(); m.insert("text".into(), json!(text)); m }
fn big(n: usize, c: char) -> String { std::iter::repeat(c).take(n).collect() }

struct World { h: Host, peers: Peers, ctl: Shm, link: Link, sc: CellsReader, mine: Cells, rec: Reclaim, name: String }

impl World {
    fn start(slots: u64) -> World {
        let name = format!("s{}x{}", std::process::id() % 100000, common::unique().rsplit('-').next().unwrap());
        let mut o = Options::new(None); o.effects = false;
        let h = Host::open(o).unwrap();
        let entry = json!({"name": name, "ring_bytes": 65536, "slots": slots, "key_cap": 128, "val_cap": VAL_CAP, "spins": SPINS, "depth": DEPTH});
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
        let peers = Peers::from_fact("membrane", &json!([entry]), None, now).unwrap();
        let ctl = Shm::attach(format!("/gl-{name}-ctl")).unwrap();
        let mine = Cells::create(format!("/gl-{name}-pc"), 8, 128, VAL_CAP, 1).unwrap();
        let sc = CellsReader::attach(format!("/gl-{name}-sc")).unwrap();
        let mut link = Link::new(Me { ns: "tester".into(), build: "t".into(), epoch: 1.0, support: vec![VERSION], initiator: false });
        link = link.with_token(String::new());
        let mut w = World { h, peers, ctl, link, sc, mine, rec: Reclaim::new(), name };
        for _ in 0..50 { w.turn(); if w.link.is_open() { break; } }
        assert!(w.link.is_open(), "the link never opened");
        let f = w.link.send(vec![Op::Sub { target: Target::Prefix("membrane::Req:".into()), props: None, limit: None }]).unwrap();
        w.ctl.send(&f).unwrap();
        w.turn();
        w
    }
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
    fn peer_reads(&mut self) { let seen = Reclaim::before(&self.sc); let _ = self.sc.scan("", SPINS, DEPTH).unwrap(); self.rec.after(seen, &self.sc, &mut self.mine, SPINS, DEPTH).unwrap(); }
    fn text(&self, eid: &str) -> Option<(CellState, J)> { self.sc.get(&key(eid, "text"), SPINS, DEPTH).ok().flatten().map(|c| (c.state, c.value)) }
    fn wait(&mut self, what: &str, ok: impl Fn(&World) -> bool) { for _ in 0..400 { self.turn(); if ok(self) { return; } } panic!("{what}"); }
}
impl Drop for World { fn drop(&mut self) { for part in ["ctl", "sc", "pc"] { let _ = graphlink_shm::remove_region(&format!("/gl-{}-{part}", self.name)); } } }

#[test]
fn a_value_over_the_tables_value_capacity_is_published_whole_and_never_refused() {
    let mut w = World::start(128);
    let v = big(39_000, 'é');   // 78,000 bytes of text: the size of the tool result that was refused on a seat with a 65,536-byte cap, here over a 1,024-byte one (about 77 pieces)
    w.h.born("membrane::Req:big", "Req", props(&v), "t");
    w.wait("the big value never reached the peer whole", |w| w.text("membrane::Req:big") == Some((CellState::Live, json!(v.clone()))));
    w.turns(20);
    assert_eq!(w.text("membrane::Req:big"), Some((CellState::Live, json!(v))), "it stays whole, and is never a refusal");
}

#[test]
fn a_big_value_changing_size_always_reads_as_the_last_one_written() {
    let mut w = World::start(64);
    w.h.born("membrane::Req:c", "Req", props(&big(9000, 'a')), "t");
    for (n, c) in [(9000usize, 'a'), (200, 'b'), (30_000, 'c'), (5000, 'd'), (10, 'e')] {
        w.h.write("membrane::Req:c", "text", json!(big(n, c)), "t");
        w.wait("the new value never reached the peer", |w| w.text("membrane::Req:c") == Some((CellState::Live, json!(big(n, c)))));
        w.peer_reads();
    }
}

#[test]
fn big_values_born_and_retired_round_after_round_pass_through_a_table_too_small_for_them_all() {
    // 40 slots; each round makes two entities of about 12 KB each (13 pieces and a head per value, and the node cell): the second round does not fit unless the pieces come back
    let mut w = World::start(40);
    for round in 0..6 {
        let ids: Vec<String> = (0..2).map(|i| format!("membrane::Req:r{round}e{i}")).collect();
        for (i, id) in ids.iter().enumerate() { w.h.born(id, "Req", props(&big(12_000 + i, 'x')), "t"); }
        w.wait(&format!("round {round}: the big values never reached the peer"), |w| ids.iter().enumerate().all(|(i, id)| w.text(id) == Some((CellState::Live, json!(big(12_000 + i, 'x'))))));
        for id in &ids { w.h.write(id, "", J::Null, "t"); }
        w.wait(&format!("round {round}: the retired entities never became Gone"), |w| ids.iter().all(|id| matches!(w.text(id), Some((CellState::Gone, _)))));
        w.peer_reads();
        w.turns(3);
    }
}

#[test]
fn a_big_value_the_table_has_no_room_for_yet_waits_and_is_published_when_slots_are_freed_not_refused_for_good() {
    // 20 slots; seven small entities take 14 (a node cell and a cell each): the big value (a node cell, about 6 pieces and a head) does not fit, though a slot or two are free
    let mut w = World::start(20);
    let small: Vec<String> = (0..7).map(|i| format!("membrane::Req:s{i}")).collect();
    for id in &small { w.h.born(id, "Req", props("x"), "t"); }
    w.wait("the small entities never became cells", |w| small.iter().all(|id| w.text(id) == Some((CellState::Live, json!("x")))));
    let v = big(6000, 'b');
    w.h.born("membrane::Req:big", "Req", props(&v), "t");
    w.turns(30);
    assert!(!matches!(w.text("membrane::Req:big"), Some((CellState::Refused, _))), "a value that waits for room is not refused for good: {:?}", w.text("membrane::Req:big").map(|t| t.0));
    for id in &small { w.h.write(id, "", J::Null, "t"); }
    w.wait("the small entities never became Gone", |w| small.iter().all(|id| matches!(w.text(id), Some((CellState::Gone, _)))));
    w.peer_reads();
    for _ in 0..400 {
        w.turn();
        w.peer_reads();   // the peer says what it has seen, so the freed slots can be given again
        if w.text("membrane::Req:big") == Some((CellState::Live, json!(v.clone()))) { return; }
    }
    panic!("the big value never reached the peer once there was room: {:?}", w.text("membrane::Req:big").map(|t| t.0));
}
