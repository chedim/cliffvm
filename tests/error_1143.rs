//! The c1f `Error` value kind and the `is` operator (#1143, docs/c1f-error.md; operator decisions 2026-11-13). What it does is tests/c1ft/error.c1ft; what stays here are the two
//! checks of Rust-side API that c1f cannot say: the reserved-name list and `truthy`.
use cliffvm::expr::{truthy, V};

#[test]
fn the_name_error_is_reserved_as_a_kind() {
    assert!(cliffvm::expr::RESERVED.contains(&"Error"));
}

#[test]
fn truthy_of_an_error_is_never_asked_by_a_getter() {
    // `truthy` is the plain-Rust helper other code calls; an Error is not true (the evaluator never reaches it: it propagates first)
    assert!(!truthy(&V::Error("timed out after 5 s; start it again with a bigger timeout_s".into())));
}
