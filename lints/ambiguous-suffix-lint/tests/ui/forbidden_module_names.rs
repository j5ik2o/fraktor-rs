#![feature(register_tool)]
#![register_tool(ambiguous_suffix)]
#![warn(ambiguous_suffix::ambiguous_suffix)]

mod helper_runtime {
  pub struct CleanRegistry;
}

pub mod system_util {
  pub struct AnotherRegistry;
}

fn main() {}
