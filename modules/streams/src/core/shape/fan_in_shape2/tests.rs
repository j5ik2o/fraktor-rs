use crate::core::shape::{FanInShape2, Inlet, Outlet};

#[test]
fn new_returns_ports_passed_at_construction() {
  let in0 = Inlet::<u32>::new();
  let in1 = Inlet::<u64>::new();
  let out = Outlet::<bool>::new();

  let in0_id = in0.id();
  let in1_id = in1.id();
  let out_id = out.id();

  let shape = FanInShape2::new(in0, in1, out);

  assert_eq!(shape.in0().id(), in0_id);
  assert_eq!(shape.in1().id(), in1_id);
  assert_eq!(shape.out().id(), out_id);
}

#[test]
fn fresh_ports_allocate_distinct_ids() {
  let shape = FanInShape2::new(Inlet::<u32>::new(), Inlet::<u64>::new(), Outlet::<bool>::new());

  assert_ne!(shape.in0().id(), shape.in1().id());
  assert_ne!(shape.in0().id().value(), shape.out().id().value());
  assert_ne!(shape.in1().id().value(), shape.out().id().value());
}

#[test]
fn copy_preserves_port_ids() {
  let shape = FanInShape2::new(Inlet::<u32>::new(), Inlet::<u64>::new(), Outlet::<bool>::new());
  let copied = shape;

  assert_eq!(shape.in0().id(), copied.in0().id());
  assert_eq!(shape.in1().id(), copied.in1().id());
  assert_eq!(shape.out().id(), copied.out().id());
}

#[test]
fn equality_holds_for_same_ports() {
  let in0 = Inlet::<u32>::new();
  let in1 = Inlet::<u64>::new();
  let out = Outlet::<bool>::new();

  let a = FanInShape2::new(in0, in1, out);
  let b = FanInShape2::new(in0, in1, out);

  assert_eq!(a, b);
}

#[test]
fn inequality_for_different_ports() {
  let a = FanInShape2::new(Inlet::<u32>::new(), Inlet::<u32>::new(), Outlet::<u32>::new());
  let b = FanInShape2::new(Inlet::<u32>::new(), Inlet::<u32>::new(), Outlet::<u32>::new());

  assert_ne!(a, b);
}
