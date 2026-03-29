use core::any::TypeId;

use crate::core::{
  graph::GraphInterpreter,
  r#impl::interpreter::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared, IslandSplitter},
};

#[test]
fn interpreter_package_contains_runtime_types() {
  // GraphInterpreter remains in graph/, boundary types are in impl/interpreter/
  let _ = TypeId::of::<GraphInterpreter>();
  let _ = TypeId::of::<IslandSplitter>();
  let _ = TypeId::of::<IslandBoundaryShared>();
  let _ = DEFAULT_BOUNDARY_CAPACITY;
}
