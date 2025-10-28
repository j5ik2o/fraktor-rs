#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

mod child {
  pub struct A;
  pub struct B;
}

pub use child::{A, B};

fn main() {}
