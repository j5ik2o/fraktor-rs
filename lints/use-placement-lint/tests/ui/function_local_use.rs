use std::fmt::Debug;

fn accept<T: Debug>(value: T) {
  use std::collections::VecDeque;

  let _ = VecDeque::<T>::from([value]);
}

fn main() {
  accept(1);
}
