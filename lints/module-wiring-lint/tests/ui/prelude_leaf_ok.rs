#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

pub mod prelude {
  pub struct PreludeItem;
}

fn main() {}
