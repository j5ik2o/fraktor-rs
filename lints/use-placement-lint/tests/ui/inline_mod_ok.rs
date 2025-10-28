use std::fmt::Debug;

pub fn do_nothing<T: Debug>(value: T) {
  let _ = value;
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use super::do_nothing;

  pub fn call() {
    do_nothing(0);
  }
}

fn main() {
  do_nothing("hello");
}
