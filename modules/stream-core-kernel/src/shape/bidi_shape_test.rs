use crate::shape::{BidiShape, FlowShape, Inlet, Outlet};

// --- from_flows construction ---

#[test]
fn from_flows_creates_bidi_shape_with_matching_ports() {
  // Given: two flow shapes
  let top_in = Inlet::<u32>::new();
  let top_out = Outlet::<u64>::new();
  let bottom_in = Inlet::<String>::new();
  let bottom_out = Outlet::<bool>::new();

  let top = FlowShape::new(top_in, top_out);
  let bottom = FlowShape::new(bottom_in, bottom_out);

  let top_in_id = top.inlet().id();
  let top_out_id = top.outlet().id();
  let bottom_in_id = bottom.inlet().id();
  let bottom_out_id = bottom.outlet().id();

  // When: creating a BidiShape from flows
  let bidi = BidiShape::from_flows(top, bottom);

  // Then: ports match the original flow shapes
  assert_eq!(bidi.top_inlet().id(), top_in_id);
  assert_eq!(bidi.top_outlet().id(), top_out_id);
  assert_eq!(bidi.bottom_inlet().id(), bottom_in_id);
  assert_eq!(bidi.bottom_outlet().id(), bottom_out_id);
}

#[test]
fn from_flows_is_equivalent_to_manual_construction() {
  // Given: ports for manual and from_flows construction
  let in1 = Inlet::<u8>::new();
  let out1 = Outlet::<u16>::new();
  let in2 = Inlet::<u32>::new();
  let out2 = Outlet::<u64>::new();

  let manual = BidiShape::new(in1, out1, in2, out2);
  let from_flows = BidiShape::from_flows(FlowShape::new(in1, out1), FlowShape::new(in2, out2));

  // Then: both produce the same shape
  assert_eq!(manual, from_flows);
}

#[test]
fn from_flows_preserves_distinct_port_ids() {
  // Given: two flow shapes with distinct ports
  let top = FlowShape::new(Inlet::<u8>::new(), Outlet::<u8>::new());
  let bottom = FlowShape::new(Inlet::<u8>::new(), Outlet::<u8>::new());

  // When: creating a BidiShape
  let bidi = BidiShape::from_flows(top, bottom);

  // Then: all four port IDs are distinct
  let ids = [
    bidi.top_inlet().id().value(),
    bidi.top_outlet().id().value(),
    bidi.bottom_inlet().id().value(),
    bidi.bottom_outlet().id().value(),
  ];
  for i in 0..ids.len() {
    for j in (i + 1)..ids.len() {
      assert_ne!(ids[i], ids[j], "port IDs at index {i} and {j} must differ");
    }
  }
}

#[test]
fn from_flows_result_is_copy() {
  // Given: a BidiShape created via from_flows
  let bidi = BidiShape::from_flows(
    FlowShape::new(Inlet::<u8>::new(), Outlet::<u16>::new()),
    FlowShape::new(Inlet::<u32>::new(), Outlet::<u64>::new()),
  );

  // When: copying the shape
  let copied = bidi;

  // Then: both are usable and equal
  assert_eq!(bidi, copied);
}
