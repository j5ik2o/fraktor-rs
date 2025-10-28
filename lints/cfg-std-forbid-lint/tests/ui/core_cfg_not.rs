#![deny(cfg_std_forbid)]

#[cfg(not(target_has_atomic = "ptr"))]
mod tests {
  #[test]
  fn dummy() {}
}

fn main() {}
