use crate::core::shape::{Inlet, Outlet, UniformFanOutShape};

#[test]
fn new_returns_ports_passed_at_construction() {
  let inlet = Inlet::<u32>::new();
  let out0 = Outlet::<u64>::new();
  let out1 = Outlet::<u64>::new();

  let inlet_id = inlet.id();
  let out0_id = out0.id();
  let out1_id = out1.id();

  let shape = UniformFanOutShape::new(inlet, vec![out0, out1]);

  assert_eq!(shape.inlet().id(), inlet_id);
  assert_eq!(shape.outlets()[0].id(), out0_id);
  assert_eq!(shape.outlets()[1].id(), out1_id);
  assert_eq!(shape.port_count(), 2);
}

#[test]
fn with_port_count_creates_requested_number_of_outlets() {
  let shape = UniformFanOutShape::<u32, u64>::with_port_count(3);

  assert_eq!(shape.port_count(), 3);
  assert_eq!(shape.outlets().len(), 3);
}
