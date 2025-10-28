use std::fmt::Debug;

fn helper<T: Debug>(value: T) {
  let _ = value;
}

#[cfg(test)]
mod tests;

fn main() {
  helper(1);
}
