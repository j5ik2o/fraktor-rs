#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

mod sample {
  pub mod domain {
    #[derive(Clone, Copy)]
    pub struct Widget;

    impl Widget {
      pub fn new() -> Self {
        Self
      }
    }
  }
}

mod other {
  #[derive(Clone, Copy)]
  pub struct Widget;
}

fn build() -> other::Widget {
  crate::sample::domain::Widget::new();
  Widget
}

use other::Widget;

fn main() {
  let _ = build();
}
