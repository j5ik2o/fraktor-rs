#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

mod sample {
  pub mod domain {
    #[derive(Clone, Copy)]
    pub struct Widget;
  }
}

use crate::sample::domain::Widget;

mod nested {
  pub fn build() -> crate::sample::domain::Widget {
    crate::sample::domain::Widget
  }
}

fn main() {
  let _ = Widget;
  let _ = nested::build();
}
