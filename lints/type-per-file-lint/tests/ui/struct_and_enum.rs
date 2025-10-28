#![feature(register_tool)]
#![register_tool(type_per_file)]
#![warn(type_per_file::multiple_type_definitions)]

pub struct SampleStruct;

pub enum SampleEnum {
  Unit,
}

fn main() {}
