use crate::core::kernel::{
  actor::messaging::AnyMessage,
  routing::{Routee, RoutingLogic},
};

// ---------------------------------------------------------------------------
// Helper: FirstRoutingLogic
// ---------------------------------------------------------------------------

/// Test implementation that always selects the first routee.
struct FirstRoutingLogic;

impl RoutingLogic for FirstRoutingLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    &routees[0]
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn custom_logic_selects_expected_routee() {
  // Given: a FirstRoutingLogic and a slice of routees
  let logic = FirstRoutingLogic;
  let routees = [Routee::NoRoutee, Routee::NoRoutee];
  let message = AnyMessage::new(1_u32);

  // When: selecting a routee
  let selected = logic.select(&message, &routees);

  // Then: it should be the first element
  assert_eq!(*selected, routees[0]);
}

#[test]
fn select_returns_reference_to_slice_element() {
  // Given: a FirstRoutingLogic and a slice of routees
  let logic = FirstRoutingLogic;
  let routees = [Routee::NoRoutee, Routee::NoRoutee];
  let message = AnyMessage::new(2_u32);

  // When: selecting a routee
  let selected = logic.select(&message, &routees);

  // Then: the returned reference should point to the same address as the slice element
  assert!(core::ptr::eq(selected, &routees[0]));
}
