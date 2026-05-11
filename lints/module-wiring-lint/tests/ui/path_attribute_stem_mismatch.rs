// compile-flags: --test

#[cfg(test)]
#[path = "other_test.rs"]
mod tests;

fn production_code() -> usize {
  1
}

fn main() {}
