// compile-flags: --test

#[cfg(test)]
#[path = "path_attribute_directory/path_attribute_directory_test.rs"]
mod tests;

fn production_code() -> usize {
  1
}

fn main() {}
