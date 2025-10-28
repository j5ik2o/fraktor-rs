#![feature(register_tool)]
#![register_tool(module_wiring)]
#![warn(module_wiring::no_parent_reexport)]

pub mod prelude {
  pub mod child {
    pub mod sub {
      pub struct Thing;
    }
  }

  pub use self::child::sub::Thing;
}

fn main() {}
