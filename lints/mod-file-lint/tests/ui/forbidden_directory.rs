#![feature(register_tool)]
#![register_tool(mod_file)]
#![warn(mod_file::no_mod_rs)]

#[path = "auxiliary/legacy/mod.rs"]
mod legacy;

fn main() {
  let _ = legacy::value();
}
