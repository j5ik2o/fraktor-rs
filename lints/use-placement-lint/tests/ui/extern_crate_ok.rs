extern crate std as std_crate;

use std_crate::fmt::Debug;

fn accept<T: Debug>(_value: T) {}

fn main() {
  accept(1);
}
