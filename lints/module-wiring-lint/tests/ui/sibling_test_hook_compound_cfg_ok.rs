// compile-flags: --test

#[cfg(all(test, unix))]
#[path = "sibling_test_hook_compound_cfg_ok_test.rs"]
mod tests;

fn production_code() -> usize {
  1
}

fn main() {}
