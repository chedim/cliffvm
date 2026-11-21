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
    fn start(slots: u64) -> World { World::start_with(slots, None) }
    fn start_with(slots: u64, pack: Option<(&str, &str)>) -> World { World::boot(slots, pack, None) }
    /// `door`: with a door secret the peer's entry says its namespace (and its `grants`, when given), and the peer's hello carries the token the seat minted for it
    fn boot(slots: u64, pack: Option<(&str, &str)>, door: Option<Option<J>>) -> World {
        let name = format!("r{}x{}", std::process::id() % 100000, common::unique().rsplit('-').next().unwrap());
        let mut o = Options::new(None); o.effects = false;
        let h = Host::open(o).unwrap();
        if let Some((n, text)) = pack { h.load_pack(n, text).unwrap(); }
        let mut entry = json!({"name": name, "ring_bytes": 65536, "slots": slots, "key_cap": 128, "val_cap": 256, "spins": SPINS, "depth": DEPTH});
        let mut auth = None;
        let mut dir = None;
        if let Some(grants) = &door {
            entry["ns"] = json!("tester");
            if let Some(g) = grants { entry["grants"] = g.clone(); }
            let d = std::env::temp_dir().join(format!("cliffvm-grants-{name}"));
            auth = Some(cliffvm::admin::PeerAuth { secret: vec![7u8; 32], dir: d.clone(), ttl: 3600.0 });
            dir = Some(d);
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
        let peers = Peers::from_fact("membrane", &json!([entry]), auth, now).unwrap();
        let ctl = Shm::attach(format!("/gl-{name}-ctl")).unwrap();
        let mine = Cells::create(format!("/gl-{name}-pc"), 8, 128, 256, 1).unwrap();   // the peer's own table: the seat reads it, and the peer's acknowledgements go in it
        let sc = CellsReader::attach(format!("/gl-{name}-sc")).unwrap();
        let mut link = Link::new(Me { ns: "tester".into(), build: "t".into(), epoch: 1.0, support: vec![VERSION], initiator: false });
        if let Some(d) = &dir { link = link.with_token(std::fs::read_to_string(d.join(format!("{name}.token"))).unwrap().trim().to_string()); let _ = std::fs::remove_dir_all(d); }
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

#[test]
fn an_accepted_write_a_peer_asks_for_creates_the_entity_of_a_class_a_pack_owns() {
    // the driver applied an accepted write as an outside write, and the host drops an outside write to an entity that does not exist when a pack owns its class: the peer got `accepted` and nothing happened.
    // The driver creates the entity (as it makes the `LinkWrite` it asks the verdict of) and then writes.
    let pack = "PACK lk 1\nLinkWrite.verdict getter: 'accept'\nOwned.kind getter: 'owned'\n";
    let mut w = World::start_with(16, Some(("lk", pack)));
    let f = w.link.send(vec![Op::Wr { wid: "w1".into(), id: "Owned:made".into(), prop: "v".into(), value: json!(7) }]).unwrap();
    w.ctl.send(&f).unwrap();
    w.wait("the write of a missing entity of an owned class never made it", |w| w.h.get("Owned:made", "v") == json!(7));
    assert_eq!(w.h.get("Owned:made", "kind"), json!("owned"), "the entity is of the class the pack owns");
}

/// a stand-in for the seat's policy (babyseat's packs/link.c1f): accept when one of the grants is a prefix of the id
const GRANT_POLICY: &str = "PACK lk 1\nLinkWrite.verdict getter: 'accept' if len([g for g in this.grants if g == '*' or startswith(this.id, g)]) > 0 else 'refused: no grant'\nLamp.kind getter: 'lamp'\nDoor.kind getter: 'door'\n";

fn peer_writes(w: &mut World, wid: &str, id: &str) {
    let f = w.link.send(vec![Op::Wr { wid: wid.into(), id: id.into(), prop: "v".into(), value: json!(1) }]).unwrap();
    w.ctl.send(&f).unwrap();
}

#[test]
fn the_grants_in_a_peers_hello_token_reach_the_write_policy() {
    let mut w = World::boot(16, Some(("lk", GRANT_POLICY)), Some(Some(json!(["Lamp:"]))));
    peer_writes(&mut w, "w1", "Lamp:a");
    peer_writes(&mut w, "w2", "Door:a");
    w.wait("the granted write never landed", |w| w.h.get("Lamp:a", "v") == json!(1));
    w.turns(10);
    assert!(w.h.get("Door:a", "v").is_null(), "a write outside the granted prefixes is refused");
}

#[test]
fn the_star_grant_means_every_prefix() {
    let mut w = World::boot(16, Some(("lk", GRANT_POLICY)), Some(Some(json!(["*"]))));
    peer_writes(&mut w, "w1", "Door:a");
    w.wait("a write under `*` never landed", |w| w.h.get("Door:a", "v") == json!(1));
}

#[test]
fn a_peer_with_no_grants_claim_has_an_empty_list() {
    // the policy accepts only when the list is empty (a missing list would be an error, not an empty one)
    let pack = "PACK lk 1\nLinkWrite.verdict getter: 'accept' if len(this.grants) == 0 else 'refused: grants'\nLamp.kind getter: 'lamp'\n";
    let mut w = World::boot(16, Some(("lk", pack)), Some(None));
    peer_writes(&mut w, "w1", "Lamp:a");
    w.wait("a peer without grants never got an empty list", |w| w.h.get("Lamp:a", "v") == json!(1));
}

#[test]
fn bad_grants_in_the_fact_stop_the_start_and_name_the_peer() {
    for bad in [json!("Lamp:"), json!([""]), json!([1]), json!(["Lamp:", ""])] {
        let entry = json!([{"name": "gbad", "ring_bytes": 65536, "slots": 8, "key_cap": 128, "val_cap": 256, "spins": SPINS, "depth": DEPTH, "ns": "tester", "grants": bad}]);
        let auth = cliffvm::admin::PeerAuth { secret: vec![7u8; 32], dir: std::env::temp_dir().join("cliffvm-grants-gbad"), ttl: 3600.0 };
        let e = Peers::from_fact("membrane", &entry, Some(auth), 0.0).err().expect("a bad grants must stop the start");
        assert!(e.contains("grants") && e.contains("gbad"), "{e}");
    }
}

// ---- a peer that subscribes by Id and lets go (the way a peer's one-shot nodes work): the slots must come back, whatever the order of retire and unsub

fn sub_id(w: &mut World, eid: &str, names: Option<Vec<&str>>) {
    let f = w.link.send(vec![Op::Sub { target: Target::Id(eid.into()), props: names.map(|n| n.into_iter().map(String::from).collect()), limit: None }]).unwrap();
    w.ctl.send(&f).unwrap();
}
fn unsub_id(w: &mut World, eid: &str) {
    let f = w.link.send(vec![Op::Unsub { target: Target::Id(eid.into()) }]).unwrap();
    w.ctl.send(&f).unwrap();
}
fn cell_state(w: &World, eid: &str, prop: &str) -> Option<CellState> { w.sc.get(&key(eid, prop), SPINS, DEPTH).ok().flatten().map(|c| c.state) }

#[test]
fn nodes_a_peer_subscribes_by_id_retires_and_unsubscribes_pass_through_a_table_too_small_for_them_all() {
    // 8 slots, two cells per node: six nodes do not fit at once. The peer subscribes the node, the node is retired, the peer unsubscribes at once (it has its answer).
    let mut w = World::start(8);
    for i in 0..6 {
        let id = format!("membrane::Cliff:k{i}");
        w.h.born(&id, "Cliff", props(i), "t");
        sub_id(&mut w, &id, None);
        w.wait(&format!("node {i}: its cells never became Live (the table is full of the nodes before it)"), |w| cell_state(w, &id, "v") == Some(CellState::Live));
        w.h.write(&id, "", J::Null, "t");   // retire
        unsub_id(&mut w, &id);
        w.wait(&format!("node {i}: its cells never became Gone after the retire and the unsub"), |w| cell_state(w, &id, "v") == Some(CellState::Gone) && cell_state(w, &id, "") == Some(CellState::Gone));
        w.peer_reads();   // the peer has seen the removals
        w.turns(3);       // the seat reads the acknowledgement and frees the slots
    }
}

#[test]
fn a_node_subscribed_by_id_that_is_retired_has_its_props_gone_too_not_only_the_node_cell() {
    let mut w = World::start(16);
    let id = "membrane::Cliff:one";
    w.h.born(id, "Cliff", props(1), "t");
    sub_id(&mut w, id, None);
    w.wait("the cells never became Live", |w| cell_state(w, id, "v") == Some(CellState::Live));
    w.h.write(id, "", J::Null, "t");   // retire; the peer stays subscribed
    w.wait("the node cell never became Gone", |w| cell_state(w, id, "") == Some(CellState::Gone));
    w.turns(5);
    assert_eq!(cell_state(&w, id, "v"), Some(CellState::Gone), "the prop cell of a retired node stays Live: its slot can never be reclaimed");
}

#[test]
fn unsubscribing_a_node_that_still_exists_makes_its_cells_gone_and_subscribing_again_brings_them_back() {
    let mut w = World::start(16);
    let id = "membrane::Cliff:two";
    w.h.born(id, "Cliff", props(7), "t");
    sub_id(&mut w, id, None);
    w.wait("the cells never became Live", |w| cell_state(w, id, "v") == Some(CellState::Live));
    unsub_id(&mut w, id);
    w.wait("an unsubscribed node's cells never became Gone (the peer no longer wants them, and their slots are held)", |w| cell_state(w, id, "v") == Some(CellState::Gone) && cell_state(w, id, "") == Some(CellState::Gone));
    sub_id(&mut w, id, None);
    w.wait("a new subscription never brought the cells back", |w| cell_state(w, id, "v") == Some(CellState::Live));
    assert_eq!(w.sc.get(&key(id, "v"), SPINS, DEPTH).unwrap().unwrap().value, json!(7));
}

#[test]
fn a_prop_the_peer_no_longer_names_becomes_gone() {
    let mut w = World::start(16);
    let id = "membrane::Cliff:three";
    let mut m = props(1); m.insert("w".into(), json!(2));
    w.h.born(id, "Cliff", m, "t");
    sub_id(&mut w, id, Some(vec!["v"]));
    w.wait("the named prop never became Live", |w| cell_state(w, id, "v") == Some(CellState::Live));
    assert_eq!(cell_state(&w, id, "w"), None, "a prop the peer did not name is not published");
    sub_id(&mut w, id, Some(vec!["w"]));   // the same target again replaces the props it named
    w.wait("the newly named prop never became Live", |w| cell_state(w, id, "w") == Some(CellState::Live));
    w.turns(5);
    assert_eq!(cell_state(&w, id, "v"), Some(CellState::Gone), "a prop no longer named keeps its slot");
}

#[test]
fn a_node_still_covered_by_a_prefix_keeps_its_cells_when_an_id_subscription_ends() {
    let mut w = World::start(16);   // the world's own subscription is the prefix `membrane::Req:`
    let id = "membrane::Req:covered";
    w.h.born(id, "Req", props(5), "t");
    sub_id(&mut w, id, None);
    w.wait("the cells never became Live", |w| cell_state(w, id, "v") == Some(CellState::Live));
    unsub_id(&mut w, id);
    w.turns(10);
    assert_eq!(cell_state(&w, id, "v"), Some(CellState::Live), "the prefix still covers it");
}
