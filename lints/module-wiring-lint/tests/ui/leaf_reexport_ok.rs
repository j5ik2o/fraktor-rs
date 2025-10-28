#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

mod child {
  pub struct Thing;
}

pub use child::Thing;

fn main() {}
