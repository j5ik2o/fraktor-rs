#![deny(cfg_std_forbid)]

#[cfg(test)]
mod tests {
  use std::time::Duration;

  #[cfg(test)]
  #[allow(unused)]
  fn helper() {
    let _ = Duration::from_millis(1);
  }
}

fn main() {}
