#![warn(no_parent_reexport)]

mod helper {
  include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ui/include_macro_violation/helper.inc"));
}

fn main() {}
