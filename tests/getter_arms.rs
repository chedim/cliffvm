//! `x if c else y` in expressions, and getter arms (the operator, 2026-11-10). What they do is tests/c1ft/getter_arms.c1ft; what stays here is the print/parse round trip of a FOR
//! block, which is a check of the Rust syntax module.
use cliffvm::syntax::{parse, parse_full, print};

const SHELF: &str = "PACK t 1
Shelf.listed on-setter books:
  ALWAYS:
    FOR b IN this.listed:
      ('Book:' + this.name + ':' + b['title']).title = b['title']
      ('Book:' + this.name + ':' + b['title']).shelf = this.eid
      FOR t IN (b['tags'] or []):
        ('Tag:' + t).seen = True
    this.count = len(this.listed or [])
";

#[test]
fn a_for_block_prints_and_parses_back_the_same() {
    let sets = parse_full(SHELF).unwrap().setters;
    assert_eq!(parse(&print(&sets)).unwrap(), sets);
}
