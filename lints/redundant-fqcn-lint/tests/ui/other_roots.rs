#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

extern crate self as fraktor_sample;

pub mod domain {
  #[derive(Clone, Copy)]
  pub struct Widget;

  impl Widget {
    pub fn new() -> Self {
      Self
    }
  }
}

mod nested {
  use crate::fraktor_sample;

  pub mod domain {
    #[derive(Clone, Copy)]
    pub enum Mode {
      Idle,
    }
  }

  pub fn build_widget() -> super::fraktor_sample::domain::Widget {
    fraktor_sample::domain::Widget::new()
  }

  pub fn is_idle(mode: domain::Mode) -> bool {
    match mode {
      self::domain::Mode::Idle => true,
    }
  }
}

fn main() {
  let _ = nested::build_widget();
  let _ = nested::is_idle(nested::domain::Mode::Idle);
}
