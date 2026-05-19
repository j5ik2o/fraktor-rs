#![feature(register_tool)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

use std::sync::atomic::AtomicBool;

fn flip(flag: &AtomicBool) {
  flag.store(true, std::sync::atomic::Ordering::Release);
}

fn main() {
  let flag = AtomicBool::new(false);
  flip(&flag);
}
