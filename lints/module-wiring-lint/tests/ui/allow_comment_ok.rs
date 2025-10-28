#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

pub mod child {
  pub mod sub {
    pub struct Thing;
    pub struct Other;
  }
}

// allow module_wiring::no_parent_reexport
pub use child::sub::Thing;

pub use child::sub::Other; // allow module_wiring::no_parent_reexport

fn main() {}
