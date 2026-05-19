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

fn helper() {
  use crate::sample::domain::Widget;
  let _ = Widget::new();
}

fn build() -> crate::sample::domain::Widget {
  crate::sample::domain::Widget::new()
}

fn main() {
  helper();
  let _ = build();
}
