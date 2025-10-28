#![feature(register_tool)]
#![register_tool(type_per_file)]
#![warn(type_per_file::multiple_type_definitions)]

struct FirstHelper;

impl FirstHelper {
  fn new() -> Self {
    Self
  }
}

struct SecondHelper;

fn main() {
  let _ = FirstHelper::new();
  let _other = SecondHelper;
}
