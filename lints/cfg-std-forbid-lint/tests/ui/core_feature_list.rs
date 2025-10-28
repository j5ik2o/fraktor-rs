#![deny(cfg_std_forbid)]

#[cfg(any(feature = "std", feature = "alloc"))]
fn mixed_guard() {}

fn main() {}
