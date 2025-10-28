#![deny(cfg_std_forbid)]

#[allow(cfg_std_forbid)]
#[cfg(feature = "std")]
fn allowed_fn() {}

fn main() {}
