#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

mod sample {
  pub mod actor {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Pid(pub u32);
  }
}

#[cfg(any(unix, windows))]
use crate::sample::actor::Pid;

#[cfg(any(unix, windows))]
fn cancel_command_count_for_actor(actor_pid: Pid, expected_pid: Pid) -> u32 {
  if actor_pid == expected_pid { 1 } else { 0 }
}

fn main() {
  #[cfg(any(unix, windows))]
  {
    let _ = cancel_command_count_for_actor(Pid(1), Pid(1));
  }
}
