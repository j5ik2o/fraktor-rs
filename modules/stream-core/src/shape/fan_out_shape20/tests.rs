use crate::shape::{FanOutShape20, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: inlet and 20 outlets with distinct element types
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let out2 = Outlet::<u32>::new();
  let out3 = Outlet::<u64>::new();
  let out4 = Outlet::<u128>::new();
  let out5 = Outlet::<i8>::new();
  let out6 = Outlet::<i16>::new();
  let out7 = Outlet::<i32>::new();
  let out8 = Outlet::<i64>::new();
  let out9 = Outlet::<i128>::new();
  let out10 = Outlet::<usize>::new();
  let out11 = Outlet::<isize>::new();
  let out12 = Outlet::<bool>::new();
  let out13 = Outlet::<char>::new();
  let out14 = Outlet::<[u8; 1]>::new();
  let out15 = Outlet::<[u8; 2]>::new();
  let out16 = Outlet::<[u8; 3]>::new();
  let out17 = Outlet::<[u8; 4]>::new();
  let out18 = Outlet::<[u8; 5]>::new();
  let out19 = Outlet::<[u8; 6]>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out19_id = out19.id();

  // When: building the shape
  let shape = FanOutShape20::new(
    inlet,
    (out0, out1, out2, out3),
    (out4, out5, out6, out7),
    (out8, out9, out10, out11),
    (out12, out13, out14, out15),
    (out16, out17, out18, out19),
  );

  // Then: first and last accessors return the original ports (template parity check)
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out19().id(), out19_id);
}

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<
    FanOutShape20<
      u32,
      u8,
      u16,
      u32,
      u64,
      u128,
      i8,
      i16,
      i32,
      i64,
      i128,
      usize,
      isize,
      bool,
      char,
      [u8; 1],
      [u8; 2],
      [u8; 3],
      [u8; 4],
      [u8; 5],
      [u8; 6],
    >,
  >();
}

#[test]
fn shape_out_associated_type_is_20_tuple() {
  // Given/When: requiring Shape::Out = (Out0, .., Out19) as a 20-tuple
  fn assert_out<
    S: Shape<
      Out = (
        u8,
        u16,
        u32,
        u64,
        u128,
        i8,
        i16,
        i32,
        i64,
        i128,
        usize,
        isize,
        bool,
        char,
        [u8; 1],
        [u8; 2],
        [u8; 3],
        [u8; 4],
        [u8; 5],
        [u8; 6],
      ),
    >,
  >() {
  }
  assert_out::<
    FanOutShape20<
      u32,
      u8,
      u16,
      u32,
      u64,
      u128,
      i8,
      i16,
      i32,
      i64,
      i128,
      usize,
      isize,
      bool,
      char,
      [u8; 1],
      [u8; 2],
      [u8; 3],
      [u8; 4],
      [u8; 5],
      [u8; 6],
    >,
  >();
}
