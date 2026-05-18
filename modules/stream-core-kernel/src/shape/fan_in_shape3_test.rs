use crate::shape::{FanInShape3, Inlet, Outlet};

#[test]
fn new_returns_ports_passed_at_construction() {
  let in0 = Inlet::<u8>::new();
  let in1 = Inlet::<u16>::new();
  let in2 = Inlet::<u32>::new();
  let out = Outlet::<u64>::new();

  let in0_id = in0.id();
  let in1_id = in1.id();
  let in2_id = in2.id();
  let out_id = out.id();

  let shape = FanInShape3::new(in0, in1, in2, out);

  assert_eq!(shape.in0().id(), in0_id);
  assert_eq!(shape.in1().id(), in1_id);
  assert_eq!(shape.in2().id(), in2_id);
  assert_eq!(shape.out().id(), out_id);
}

#[test]
fn copy_preserves_port_ids() {
  let shape = FanInShape3::new(Inlet::<u8>::new(), Inlet::<u16>::new(), Inlet::<u32>::new(), Outlet::<u64>::new());
  let copied = shape;

  assert_eq!(shape.in0().id(), copied.in0().id());
  assert_eq!(shape.in1().id(), copied.in1().id());
  assert_eq!(shape.in2().id(), copied.in2().id());
  assert_eq!(shape.out().id(), copied.out().id());
}
