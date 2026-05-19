#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

mod sample {
  pub mod domain {
    #[derive(Clone, Copy)]
    pub struct Widget;
  }
}

type Alias = crate::sample::domain::Widget;

fn accepts(_value: crate::sample::domain::Widget) -> crate::sample::domain::Widget {
  crate::sample::domain::Widget
}

struct Holder {
  value: crate::sample::domain::Widget,
}

fn main() {
  let value = accepts(crate::sample::domain::Widget);
  let _holder = Holder { value };
  let _alias: Alias = value;
}
