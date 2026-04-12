#![feature(register_tool)]
#![register_tool(actor_lock_construction)]
#![warn(actor_lock_construction::actor_lock_construction)]

pub struct CleanType;

impl CleanType {
  pub fn build(value: usize) -> Self {
    let _ = value;
    Self
  }
}

fn main() {}
