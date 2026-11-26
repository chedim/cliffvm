//! `Error` in the static inference (#1143, docs/c1f-error.md section (d)): Error is a KIND (not a null flag, never stripped by `or`); an operation
//! on a value that may be an Error may be an Error; `x is Error` and `why(x)` read it without; the failure sites type as `T | Error`.
use cliffvm::infer::{infer_packs, infer_src};

fn ty(src: &str, vars: &[(&str, &str)]) -> String { infer_src(src, vars).unwrap().0 }
fn found(src: &str, vars: &[(&str, &str)]) -> Vec<String> { infer_src(src, vars).unwrap().1 }

#[test]
fn a_getter_that_can_fail_types_as_t_or_error() {
    let r = infer_packs(&["PACK p 1\nBox.o getter:\n  WHEN this.bad: Error('it failed; fix it')\n  ELSE: 'fine'\nBox.n getter: Int(this.raw)\nBox.plus getter: this.o + '!'\nBox.chk getter: this.o is Error\n"]).unwrap();
    let g = &r[0].1.getters;
    assert_eq!(g["Box.o"], "Text|Error");
    assert_eq!(g["Box.plus"], "Text|Error");
    assert_eq!(g["Box.chk"], "Bool");
    assert_eq!(g["Box.n"], "(Int|Error)?");
}
