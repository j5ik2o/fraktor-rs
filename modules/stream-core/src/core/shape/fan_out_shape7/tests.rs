use crate::core::shape::{FanOutShape7, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: inlet and 7 outlets with distinct element types
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u32>::new();
  let out3 = Outlet::<u64>::new();
  let out4 = Outlet::<u128>::new();
  let out5 = Outlet::<i8>::new();
  let out6 = Outlet::<i16>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out6_id = out6.id();

  // When: building the shape
  let shape = FanOutShape7::new(inlet, (out0, out1, out2, out3), (out4, out5, out6));

  // Then: first and last accessors return the original ports (template parity check)
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out6().id(), out6_id);
}

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<FanOutShape7<u32, u8, u16, u32, u64, u128, i8, i16>>();
}

#[test]
fn shape_out_associated_type_is_7_tuple() {
  // Given/When: requiring Shape::Out = (Out0, .., Out6) as a 7-tuple
  fn assert_out<S: Shape<Out = (u8, u16, u32, u64, u128, i8, i16)>>() {}
  assert_out::<FanOutShape7<u32, u8, u16, u32, u64, u128, i8, i16>>();
}
