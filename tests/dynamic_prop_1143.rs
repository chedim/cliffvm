//! A write whose PROPERTY NAME is computed (#1143, the MCP door's entity_write): `<target>.[<expr>] = value`. The expression gives the
//! property's name (non-empty text; anything else writes nothing). Everything else is a write as before: the target's class must be
//! one the pack owns or imports (a receipt `refused` otherwise), and the entity is created by the creation path.
//! What it does is tests/c1ft/dynamic_prop.c1ft; what stays here is the print round trip.
use cliffvm::syntax::{parse, print};

#[test]
fn it_prints_back_the_same_text() {
    let src = "PACK t 1\nBox.go on-setter t:\n  ALWAYS:\n    (this.target).[this.field] = this.v\n";
    let again = print(&parse(src).unwrap());
    assert!(again.contains("(this.target).[this.field] = this.v"), "{again}");
}
