#![feature(register_tool)]
#![register_tool(mod_file)]
#![allow(no_mod_rs)]
#![allow(mod_file::no_mod_rs)]

#[path = "auxiliary/suppressed/mod.rs"]
mod suppressed;

fn main() {
  let _ = suppressed::value();
}
