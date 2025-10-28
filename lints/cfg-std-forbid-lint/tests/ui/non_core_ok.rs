#![allow(cfg_std_forbid)]

#[cfg(feature = "std")]
fn uses_std() {}

use std::thread;

fn main() {
  thread::yield_now();
}
