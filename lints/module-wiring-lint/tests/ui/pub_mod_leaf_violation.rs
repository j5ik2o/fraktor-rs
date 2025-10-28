#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

pub mod leaf {
  pub struct Leaf;
}

fn main() {}
