#![feature(register_tool)]
#![register_tool(module_examples_lint)]
#![warn(module_examples_forbid)]

#[path = "auxiliary/modules/actor-adaptor/examples/classic_logging.rs"]
mod classic_logging;

fn main() {
  let _ = classic_logging::value();
}
