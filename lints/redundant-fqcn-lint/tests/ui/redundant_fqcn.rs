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

    #[derive(Clone, Copy)]
    pub enum Mode {
      Idle,
    }
  }
}

fn build_widget() -> sample::domain::Widget {
  crate::sample::domain::Widget::new()
}

fn is_idle(mode: sample::domain::Mode) -> bool {
  match mode {
    crate::sample::domain::Mode::Idle => true,
  }
}

fn main() {
  let _ = build_widget();
}
