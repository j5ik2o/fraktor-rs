#![feature(register_tool)]
#![register_tool(type_per_file)]
#![warn(type_per_file::multiple_type_definitions)]

pub struct BaseType;

#[allow(multiple_type_definitions)]
#[allow(type_per_file::multiple_type_definitions)]
pub enum AllowedEnum {
  Case,
}

fn main() {}
