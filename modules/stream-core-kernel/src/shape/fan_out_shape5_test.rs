use crate::shape::{FanOutShape5, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: inlet and 5 outlets with distinct element types
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u32>::new();
  let out3 = Outlet::<u64>::new();
  let out4 = Outlet::<u128>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out4_id = out4.id();

  // When: building the shape
  let shape = FanOutShape5::new(inlet, out0, out1, out2, out3, out4);

  // Then: first and last accessors return the original ports (template parity check)
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out4().id(), out4_id);
}

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<FanOutShape5<u32, u8, u16, u32, u64, u128>>();
}

#[test]
fn shape_out_associated_type_is_5_tuple() {
  // Given/When: requiring Shape::Out = (Out0, .., Out4) as a 5-tuple
  fn assert_out<S: Shape<Out = (u8, u16, u32, u64, u128)>>() {}
  assert_out::<FanOutShape5<u32, u8, u16, u32, u64, u128>>();
}
