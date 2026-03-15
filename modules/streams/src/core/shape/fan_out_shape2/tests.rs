use crate::core::shape::{FanOutShape2, Inlet, Outlet};

#[test]
fn new_returns_ports_passed_at_construction() {
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u64>::new();
  let out1 = Outlet::<bool>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out1_id = out1.id();

  let shape = FanOutShape2::new(inlet, out0, out1);

  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out1().id(), out1_id);
}

#[test]
fn fresh_ports_allocate_distinct_ids() {
  let shape = FanOutShape2::new(Inlet::<u32>::new(), Outlet::<u64>::new(), Outlet::<bool>::new());

  assert_ne!(shape.inlet().id().value(), shape.out0().id().value());
  assert_ne!(shape.inlet().id().value(), shape.out1().id().value());
  assert_ne!(shape.out0().id().value(), shape.out1().id().value());
}

#[test]
fn copy_preserves_port_ids() {
  let shape = FanOutShape2::new(Inlet::<u32>::new(), Outlet::<u64>::new(), Outlet::<bool>::new());
  let copied = shape;

  assert_eq!(shape.inlet().id(), copied.inlet().id());
  assert_eq!(shape.out0().id(), copied.out0().id());
  assert_eq!(shape.out1().id(), copied.out1().id());
}

#[test]
fn equality_holds_for_same_ports() {
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u64>::new();
  let out1 = Outlet::<bool>::new();

  let a = FanOutShape2::new(inlet, out0, out1);
  let b = FanOutShape2::new(inlet, out0, out1);

  assert_eq!(a, b);
}

#[test]
fn inequality_for_different_ports() {
  let a = FanOutShape2::new(Inlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new());
  let b = FanOutShape2::new(Inlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new());

  assert_ne!(a, b);
}

/// Verifies that `FanOutShape2` supports heterogeneous output types,
/// which distinguishes it from `UniformFanOutShape` (where all outputs
/// share the same type, as used by `Partition` and `Broadcast` in Pekko).
#[test]
fn heterogeneous_output_types_are_supported() {
  let shape = FanOutShape2::new(Inlet::<u32>::new(), Outlet::<String>::new(), Outlet::<bool>::new());

  let _out0: &Outlet<String> = shape.out0();
  let _out1: &Outlet<bool> = shape.out1();
}
