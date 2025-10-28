#![deny(cfg_std_forbid)]

#[cfg(all(test, feature = "std"))]
fn uses_std() {}

fn main() {}
