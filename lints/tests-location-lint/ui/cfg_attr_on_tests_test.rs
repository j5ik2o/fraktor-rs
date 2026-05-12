// normalize-stderr-test: "(\n)\n\z" -> "$1"
// compile-flags: --test
#![cfg(test)]

#[test]
fn detect_inner_cfg() {}
