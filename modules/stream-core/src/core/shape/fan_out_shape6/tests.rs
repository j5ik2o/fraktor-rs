use crate::core::shape::{FanOutShape6, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: inlet and 6 outlets with distinct element types
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u32>::new();
  let out3 = Outlet::<u64>::new();
  let out4 = Outlet::<u128>::new();
  let out5 = Outlet::<i8>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out5_id = out5.id();

  // When: building the shape
  let shape = FanOutShape6::new(inlet, out0, out1, out2, out3, out4, out5);

  // Then: first and last accessors return the original ports (template parity check)
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out5().id(), out5_id);
}

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<FanOutShape6<u32, u8, u16, u32, u64, u128, i8>>();
}

#[test]
fn shape_out_associated_type_is_6_tuple() {
  // Given/When: requiring Shape::Out = (Out0, .., Out5) as a 6-tuple
  fn assert_out<S: Shape<Out = (u8, u16, u32, u64, u128, i8)>>() {}
  assert_out::<FanOutShape6<u32, u8, u16, u32, u64, u128, i8>>();
}
