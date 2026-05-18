#![warn(separate_tests)]

struct Foo;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_inline_is_rejected() {}
}

fn main() {}
