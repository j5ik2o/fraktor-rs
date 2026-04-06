use core::any::TypeId;

use crate::core::r#impl::interpreter::{
  DEFAULT_BOUNDARY_CAPACITY, GraphInterpreter, IslandBoundaryShared, IslandSplitter,
};

#[test]
fn interpreter_package_contains_runtime_types() {
  // GraphInterpreter and boundary types are in impl/interpreter/
  let _ = TypeId::of::<GraphInterpreter>();
  let _ = TypeId::of::<IslandSplitter>();
  let _ = TypeId::of::<IslandBoundaryShared>();
  let _ = DEFAULT_BOUNDARY_CAPACITY;
}
