use crate::shape::{FanInShape1, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: freshly allocated input and output ports with stable ids
  let in0 = Inlet::<u32>::new();
  let out = Outlet::<bool>::new();

  let in0_id = in0.id();
  let out_id = out.id();

  // When: building the FanInShape1 from those ports
  let shape = FanInShape1::new(in0, out);

  // Then: accessor return values preserve the originally supplied port identities
  assert_eq!(shape.in0().id(), in0_id);
  assert_eq!(shape.out().id(), out_id);
}

#[test]
fn fresh_ports_allocate_distinct_ids() {
  // Given/When: a FanInShape1 built from independently allocated ports
  let shape = FanInShape1::new(Inlet::<u32>::new(), Outlet::<bool>::new());

  // Then: the inlet and outlet have distinct port ids
  assert_ne!(shape.in0().id().value(), shape.out().id().value());
}

#[test]
fn copy_preserves_port_ids() {
  // Given: a FanInShape1 instance
  let shape = FanInShape1::new(Inlet::<u32>::new(), Outlet::<bool>::new());

  // When: copying via Copy semantics (parity with FanInShape2)
  let copied = shape;

  // Then: the copy retains the source's port identities
  assert_eq!(shape.in0().id(), copied.in0().id());
  assert_eq!(shape.out().id(), copied.out().id());
}

#[test]
fn equality_holds_for_same_ports() {
  // Given: two shapes built from the same underlying port instances
  let in0 = Inlet::<u32>::new();
  let out = Outlet::<bool>::new();

  let a = FanInShape1::new(in0, out);
  let b = FanInShape1::new(in0, out);

  // Then: they compare equal under PartialEq
  assert_eq!(a, b);
}

#[test]
fn inequality_for_different_ports() {
  // Given: two shapes built from independently allocated ports
  let a = FanInShape1::new(Inlet::<u32>::new(), Outlet::<u32>::new());
  let b = FanInShape1::new(Inlet::<u32>::new(), Outlet::<u32>::new());

  // Then: they are NOT equal because their port ids differ
  assert_ne!(a, b);
}

// --- Shape trait contract (Pekko parity: FanInShape1[-T0, +O]) ---

#[test]
fn shape_in_associated_type_is_single_inlet_payload() {
  // Given/When: requiring Shape::In = In0 statically
  //   Pekko reference: FanInShape1[T0, O] — single input, single output.
  //   In Rust we expose the input type unwrapped (NOT a tuple, since there is only one inlet).
  fn assert_in_is_single<S: Shape<In = u32>>() {}
  assert_in_is_single::<FanInShape1<u32, bool>>();
}

#[test]
fn shape_out_associated_type_is_outlet_payload() {
  // Given/When: requiring Shape::Out = Out statically
  fn assert_out<S: Shape<Out = bool>>() {}
  assert_out::<FanInShape1<u32, bool>>();
}
