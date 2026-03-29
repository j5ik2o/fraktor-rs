use core::any::TypeId;

use crate::core::{
  graph::{DEFAULT_BOUNDARY_CAPACITY, GraphInterpreter, IslandSplitter},
  r#impl,
};

#[test]
fn interpreter_package_reexports_graph_runtime_types() {
  assert_eq!(TypeId::of::<r#impl::GraphInterpreter>(), TypeId::of::<GraphInterpreter>());
  assert_eq!(TypeId::of::<r#impl::IslandSplitter>(), TypeId::of::<IslandSplitter>());
  assert_eq!(r#impl::DEFAULT_BOUNDARY_CAPACITY, DEFAULT_BOUNDARY_CAPACITY);
}
