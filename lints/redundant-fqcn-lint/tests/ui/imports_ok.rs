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

use crate::sample::domain::{Mode, Widget};

fn build_widget() -> Widget {
  Widget::new()
}

fn is_idle(mode: Mode) -> bool {
  matches!(mode, Mode::Idle)
}

fn main() {
  let _ = build_widget();
  let _ = is_idle(Mode::Idle);
}
