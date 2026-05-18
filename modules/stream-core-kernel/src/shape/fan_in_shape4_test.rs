use crate::shape::{FanInShape4, Inlet, Outlet};

#[test]
fn new_returns_ports_passed_at_construction() {
  let in0 = Inlet::<u8>::new();
  let in1 = Inlet::<u16>::new();
  let in2 = Inlet::<u32>::new();
  let in3 = Inlet::<u64>::new();
  let out = Outlet::<bool>::new();

  let in0_id = in0.id();
  let in1_id = in1.id();
  let in2_id = in2.id();
  let in3_id = in3.id();
  let out_id = out.id();

  let shape = FanInShape4::new(in0, in1, in2, in3, out);

  assert_eq!(shape.in0().id(), in0_id);
  assert_eq!(shape.in1().id(), in1_id);
  assert_eq!(shape.in2().id(), in2_id);
  assert_eq!(shape.in3().id(), in3_id);
  assert_eq!(shape.out().id(), out_id);
}
