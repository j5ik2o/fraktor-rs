use alloc::string::String;

use crate::shape::{FanOutShape3, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: freshly allocated inlet and three outlets with stable ids
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u64>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out1_id = out1.id();
  let out2_id = out2.id();

  // When: building the shape from those ports
  let shape = FanOutShape3::new(inlet, out0, out1, out2);

  // Then: every accessor returns the originally supplied port identity
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out1().id(), out1_id);
  assert_eq!(shape.out2().id(), out2_id);
}

#[test]
fn fresh_ports_allocate_distinct_ids() {
  // Given/When: a shape built from independently allocated ports
  let shape = FanOutShape3::new(Inlet::<u32>::new(), Outlet::<u8>::new(), Outlet::<u16>::new(), Outlet::<u64>::new());

  // Then: each pair of ports has a distinct id
  assert_ne!(shape.inlet().id().value(), shape.out0().id().value());
  assert_ne!(shape.inlet().id().value(), shape.out1().id().value());
  assert_ne!(shape.inlet().id().value(), shape.out2().id().value());
  assert_ne!(shape.out0().id().value(), shape.out1().id().value());
  assert_ne!(shape.out0().id().value(), shape.out2().id().value());
  assert_ne!(shape.out1().id().value(), shape.out2().id().value());
}

#[test]
fn copy_preserves_port_ids() {
  // Given: a shape instance
  let shape = FanOutShape3::new(Inlet::<u32>::new(), Outlet::<u8>::new(), Outlet::<u16>::new(), Outlet::<u64>::new());

  // When: bit-copied via Copy semantics (parity with FanOutShape2)
  let copied = shape;

  // Then: the copy retains the source's port identities
  assert_eq!(shape.inlet().id(), copied.inlet().id());
  assert_eq!(shape.out0().id(), copied.out0().id());
  assert_eq!(shape.out1().id(), copied.out1().id());
  assert_eq!(shape.out2().id(), copied.out2().id());
}

#[test]
fn equality_holds_for_same_ports() {
  // Given: two shapes built from the same underlying port instances
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u64>::new();

  let a = FanOutShape3::new(inlet, out0, out1, out2);
  let b = FanOutShape3::new(inlet, out0, out1, out2);

  // Then: they compare equal under PartialEq
  assert_eq!(a, b);
}

#[test]
fn inequality_for_different_ports() {
  // Given: two shapes built from independently allocated ports
  let a = FanOutShape3::new(Inlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new());
  let b = FanOutShape3::new(Inlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new(), Outlet::<u32>::new());

  // Then: they are NOT equal because their port ids differ
  assert_ne!(a, b);
}

// --- Shape trait contract (Pekko parity: FanOutShape3[In, Out0, Out1, Out2]) ---

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<FanOutShape3<u32, u8, u16, u64>>();
}

#[test]
fn shape_out_associated_type_is_tuple_of_outlets() {
  // Given/When: requiring Shape::Out = (Out0, Out1, Out2) statically
  //   Pekko reference: FanOutShape3[In, Out0, Out1, Out2].
  //   In Rust we expose the output types as a tuple at the Shape trait level.
  fn assert_out<S: Shape<Out = (u8, u16, u64)>>() {}
  assert_out::<FanOutShape3<u32, u8, u16, u64>>();
}

#[test]
fn heterogeneous_output_types_are_supported() {
  // Given: three outputs with distinct types
  let shape =
    FanOutShape3::new(Inlet::<u32>::new(), Outlet::<String>::new(), Outlet::<bool>::new(), Outlet::<u64>::new());

  // Then: each outlet preserves its element type statically
  let _out0: &Outlet<String> = shape.out0();
  let _out1: &Outlet<bool> = shape.out1();
  let _out2: &Outlet<u64> = shape.out2();
}
