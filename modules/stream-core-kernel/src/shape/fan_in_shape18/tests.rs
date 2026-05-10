use crate::shape::{FanInShape18, Inlet, Outlet};

#[test]
fn new_returns_ports_passed_at_construction() {
  let in0 = Inlet::<u8>::new();
  let in1 = Inlet::<u16>::new();
  let in2 = Inlet::<u32>::new();
  let in3 = Inlet::<u64>::new();
  let in4 = Inlet::<u128>::new();
  let in5 = Inlet::<i8>::new();
  let in6 = Inlet::<i16>::new();
  let in7 = Inlet::<i32>::new();
  let in8 = Inlet::<i64>::new();
  let in9 = Inlet::<i128>::new();
  let in10 = Inlet::<usize>::new();
  let in11 = Inlet::<isize>::new();
  let in12 = Inlet::<bool>::new();
  let in13 = Inlet::<char>::new();
  let in14 = Inlet::<[u8; 1]>::new();
  let in15 = Inlet::<[u8; 2]>::new();
  let in16 = Inlet::<[u8; 3]>::new();
  let in17 = Inlet::<[u8; 4]>::new();
  let out = Outlet::<[u8; 5]>::new();

  let in0_id = in0.id();
  let in1_id = in1.id();
  let in2_id = in2.id();
  let in3_id = in3.id();
  let in4_id = in4.id();
  let in5_id = in5.id();
  let in6_id = in6.id();
  let in7_id = in7.id();
  let in8_id = in8.id();
  let in9_id = in9.id();
  let in10_id = in10.id();
  let in11_id = in11.id();
  let in12_id = in12.id();
  let in13_id = in13.id();
  let in14_id = in14.id();
  let in15_id = in15.id();
  let in16_id = in16.id();
  let in17_id = in17.id();
  let out_id = out.id();

  let shape = FanInShape18::new(
    (in0, in1, in2, in3),
    (in4, in5, in6, in7),
    (in8, in9, in10, in11),
    (in12, in13, in14, in15),
    (in16, in17),
    out,
  );

  assert_eq!(shape.in0().id(), in0_id);
  assert_eq!(shape.in1().id(), in1_id);
  assert_eq!(shape.in2().id(), in2_id);
  assert_eq!(shape.in3().id(), in3_id);
  assert_eq!(shape.in4().id(), in4_id);
  assert_eq!(shape.in5().id(), in5_id);
  assert_eq!(shape.in6().id(), in6_id);
  assert_eq!(shape.in7().id(), in7_id);
  assert_eq!(shape.in8().id(), in8_id);
  assert_eq!(shape.in9().id(), in9_id);
  assert_eq!(shape.in10().id(), in10_id);
  assert_eq!(shape.in11().id(), in11_id);
  assert_eq!(shape.in12().id(), in12_id);
  assert_eq!(shape.in13().id(), in13_id);
  assert_eq!(shape.in14().id(), in14_id);
  assert_eq!(shape.in15().id(), in15_id);
  assert_eq!(shape.in16().id(), in16_id);
  assert_eq!(shape.in17().id(), in17_id);
  assert_eq!(shape.out().id(), out_id);
}
