// compile-flags: --test

#[cfg(test)]
#[path = "sibling_test_hook_ok_test.rs"]
mod tests;

fn production_code() -> usize {
  1
}

fn main() {}
