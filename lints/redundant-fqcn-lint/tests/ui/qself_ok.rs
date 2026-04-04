#![feature(register_tool)]
#![feature(more_qualified_paths)]
#![register_tool(redundant_fqcn)]
#![warn(redundant_fqcn::redundant_fqcn)]

trait BuildPath {
  type Output;
}

struct Factory;

impl BuildPath for Factory {
  type Output = sample::domain::Widget;
}

mod sample {
  pub mod domain {
    pub struct Widget {
      pub value: i32,
    }
  }
}

fn make_widget() -> <Factory as BuildPath>::Output {
  <Factory as BuildPath>::Output { value: 1 }
}

fn main() {
  let _ = make_widget();
}
