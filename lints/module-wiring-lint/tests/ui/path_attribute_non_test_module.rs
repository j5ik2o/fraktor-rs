// compile-flags: --test

#[cfg(test)]
#[path = "path_attribute_non_test_module_test.rs"]
mod helper;

fn production_code() -> usize {
  1
}

fn main() {}
