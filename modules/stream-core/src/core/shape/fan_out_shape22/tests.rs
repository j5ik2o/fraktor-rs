use crate::core::shape::{FanOutShape22, Inlet, Outlet, Shape};

#[test]
fn new_returns_ports_passed_at_construction() {
  // Given: one inlet and 22 outlets, each with a distinct element type to
  //   guarantee independent PortId allocation.
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
  let out20 = Outlet::<[u8; 7]>::new();
  let out21 = Outlet::<[u8; 8]>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out1_id = out1.id();
  let out2_id = out2.id();
  let out3_id = out3.id();
  let out4_id = out4.id();
  let out5_id = out5.id();
  let out6_id = out6.id();
  let out7_id = out7.id();
  let out8_id = out8.id();
  let out9_id = out9.id();
  let out10_id = out10.id();
  let out11_id = out11.id();
  let out12_id = out12.id();
  let out13_id = out13.id();
  let out14_id = out14.id();
  let out15_id = out15.id();
  let out16_id = out16.id();
  let out17_id = out17.id();
  let out18_id = out18.id();
  let out19_id = out19.id();
  let out20_id = out20.id();
  let out21_id = out21.id();

  // When: building the 22-output shape
  let shape = FanOutShape22::new(
    inlet,
    (out0, out1, out2, out3),
    (out4, out5, out6, out7),
    (out8, out9, out10, out11),
    (out12, out13, out14, out15),
    (out16, out17, out18, out19),
    (out20, out21),
  );

  // Then: every accessor returns the originally supplied port identity
  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.out0().id(), out0_id);
  assert_eq!(shape.out1().id(), out1_id);
  assert_eq!(shape.out2().id(), out2_id);
  assert_eq!(shape.out3().id(), out3_id);
  assert_eq!(shape.out4().id(), out4_id);
  assert_eq!(shape.out5().id(), out5_id);
  assert_eq!(shape.out6().id(), out6_id);
  assert_eq!(shape.out7().id(), out7_id);
  assert_eq!(shape.out8().id(), out8_id);
  assert_eq!(shape.out9().id(), out9_id);
  assert_eq!(shape.out10().id(), out10_id);
  assert_eq!(shape.out11().id(), out11_id);
  assert_eq!(shape.out12().id(), out12_id);
  assert_eq!(shape.out13().id(), out13_id);
  assert_eq!(shape.out14().id(), out14_id);
  assert_eq!(shape.out15().id(), out15_id);
  assert_eq!(shape.out16().id(), out16_id);
  assert_eq!(shape.out17().id(), out17_id);
  assert_eq!(shape.out18().id(), out18_id);
  assert_eq!(shape.out19().id(), out19_id);
  assert_eq!(shape.out20().id(), out20_id);
  assert_eq!(shape.out21().id(), out21_id);
}

#[test]
fn copy_preserves_port_ids_across_all_outlets() {
  // Given: a fully populated shape
  let shape = FanOutShape22::new(
    Inlet::<u32>::new(),
    (Outlet::<u8>::new(), Outlet::<u16>::new(), Outlet::<u32>::new(), Outlet::<u64>::new()),
    (Outlet::<u128>::new(), Outlet::<i8>::new(), Outlet::<i16>::new(), Outlet::<i32>::new()),
    (Outlet::<i64>::new(), Outlet::<i128>::new(), Outlet::<usize>::new(), Outlet::<isize>::new()),
    (Outlet::<bool>::new(), Outlet::<char>::new(), Outlet::<[u8; 1]>::new(), Outlet::<[u8; 2]>::new()),
    (Outlet::<[u8; 3]>::new(), Outlet::<[u8; 4]>::new(), Outlet::<[u8; 5]>::new(), Outlet::<[u8; 6]>::new()),
    (Outlet::<[u8; 7]>::new(), Outlet::<[u8; 8]>::new()),
  );

  // When: bit-copied
  let copied = shape;

  // Then: every corresponding port id matches
  assert_eq!(shape.inlet().id(), copied.inlet().id());
  assert_eq!(shape.out0().id(), copied.out0().id());
  assert_eq!(shape.out21().id(), copied.out21().id());
  // Sanity-check a few middle indices to ensure consistent layout
  assert_eq!(shape.out10().id(), copied.out10().id());
  assert_eq!(shape.out11().id(), copied.out11().id());
  assert_eq!(shape.out15().id(), copied.out15().id());
}

#[test]
fn shape_in_associated_type_is_inlet_payload() {
  // Given/When: requiring Shape::In = In statically
  fn assert_in<S: Shape<In = u32>>() {}
  assert_in::<
    FanOutShape22<
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
      [u8; 7],
      [u8; 8],
    >,
  >();
}

#[test]
fn shape_out_associated_type_is_22_tuple() {
  // Given/When: requiring Shape::Out = (Out0, .., Out21) as a 22-tuple
  type Outs = (
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
    [u8; 7],
    [u8; 8],
  );
  fn assert_out<S: Shape<Out = Outs>>() {}
  assert_out::<
    FanOutShape22<
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
      [u8; 7],
      [u8; 8],
    >,
  >();
}
