#![warn(no_parent_reexport)]

#[path = "path_attribute_violation/leaf.module"]
mod leaf;

fn use_leaf() -> leaf::Leaf {
  leaf::Leaf
}

fn main() {
  let _ = use_leaf();
}
