#![feature(register_tool)]
#![register_tool(type_per_file)]
#![warn(type_per_file::multiple_type_definitions)]

pub struct PublicType;

impl PublicType {
  pub fn new() -> Self {
    Self
  }
}

struct PrivateHelper;

fn main() {
  let _ = PublicType::new();
  let _ = PrivateHelper;
}
