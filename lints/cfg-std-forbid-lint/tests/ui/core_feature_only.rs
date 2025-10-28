#![deny(cfg_std_forbid)]

#[cfg(feature = "std")]
fn std_only() {}

fn main() {}
