#![feature(register_tool)]
#![register_tool(module_examples_lint)]
#![warn(module_examples_forbid)]

#[path = "auxiliary/showcases/std/getting_started/main.rs"]
mod getting_started;

fn main() {
  let _ = getting_started::value();
}
