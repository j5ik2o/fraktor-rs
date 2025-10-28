#![feature(register_tool)]
#![register_tool(type_per_file)]
#![warn(type_per_file::multiple_type_definitions)]

pub struct FirstType;

impl FirstType {
  pub fn new() -> Self {
    Self
  }
}

fn main() {}
