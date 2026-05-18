use crate::{
  attributes::Attributes,
  snapshot::{ConnectionSnapshot, ConnectionState, LogicSnapshot},
};

// Local helper: build a LogicSnapshot with the given index/label.
fn logic(index: u32, label: &'static str) -> LogicSnapshot {
  LogicSnapshot::new(index, label, Attributes::new())
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_preserves_id() {
  // Given: a connection snapshot with id 7
  let conn = ConnectionSnapshot::new(7, logic(0, "upstream"), logic(1, "downstream"), ConnectionState::ShouldPull);

  // Then: id() returns 7
  assert_eq!(conn.id(), 7);
}

#[test]
fn new_preserves_in_logic() {
  // Given: an upstream logic with index 0 and label "upstream"
  let conn = ConnectionSnapshot::new(0, logic(0, "upstream"), logic(1, "downstream"), ConnectionState::ShouldPull);

  // Then: in_logic() exposes the upstream logic (renamed from Pekko's `in`)
  assert_eq!(conn.in_logic().index(), 0);
  assert_eq!(conn.in_logic().label(), "upstream");
}

#[test]
fn new_preserves_out() {
  // Given: a downstream logic with index 1 and label "downstream"
  let conn = ConnectionSnapshot::new(0, logic(0, "upstream"), logic(1, "downstream"), ConnectionState::ShouldPull);

  // Then: out() exposes the downstream logic
  assert_eq!(conn.out().index(), 1);
  assert_eq!(conn.out().label(), "downstream");
}

#[test]
fn new_preserves_state_should_pull() {
  // Given: a connection initialised with ShouldPull
  let conn = ConnectionSnapshot::new(0, logic(0, "u"), logic(1, "d"), ConnectionState::ShouldPull);

  // Then: state() returns ShouldPull
  assert_eq!(conn.state(), ConnectionState::ShouldPull);
}

#[test]
fn new_preserves_state_should_push() {
  // Given: a connection initialised with ShouldPush
  let conn = ConnectionSnapshot::new(0, logic(0, "u"), logic(1, "d"), ConnectionState::ShouldPush);

  // Then: state() returns ShouldPush
  assert_eq!(conn.state(), ConnectionState::ShouldPush);
}

#[test]
fn new_preserves_state_closed() {
  // Given: a connection initialised with Closed
  let conn = ConnectionSnapshot::new(0, logic(0, "u"), logic(1, "d"), ConnectionState::Closed);

  // Then: state() returns Closed
  assert_eq!(conn.state(), ConnectionState::Closed);
}

// ---------------------------------------------------------------------------
// Boundary values
// ---------------------------------------------------------------------------

#[test]
fn new_accepts_u32_max_id() {
  // Given: the maximum u32 id
  let conn = ConnectionSnapshot::new(u32::MAX, logic(0, "u"), logic(1, "d"), ConnectionState::Closed);

  // Then: id() returns u32::MAX
  assert_eq!(conn.id(), u32::MAX);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_preserves_all_fields() {
  // Given: a connection snapshot
  let original = ConnectionSnapshot::new(9, logic(3, "up"), logic(4, "down"), ConnectionState::ShouldPush);

  // When: cloned
  let cloned = original.clone();

  // Then: every field is preserved
  assert_eq!(cloned.id(), 9);
  assert_eq!(cloned.in_logic().index(), 3);
  assert_eq!(cloned.out().index(), 4);
  assert_eq!(cloned.state(), ConnectionState::ShouldPush);
}

#[test]
fn debug_format_contains_id_and_state() {
  // Given: a connection snapshot
  let conn = ConnectionSnapshot::new(42, logic(0, "u"), logic(1, "d"), ConnectionState::Closed);

  // When: formatted with Debug
  let debug = alloc::format!("{conn:?}");

  // Then: both the id and the state discriminator appear
  assert!(debug.contains("42"));
  assert!(debug.contains("Closed"));
}
