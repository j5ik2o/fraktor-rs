use super::OfferOutcome;

#[test]
fn offer_outcome_enqueued_variant() {
  let outcome = OfferOutcome::Enqueued;
  assert_eq!(outcome, OfferOutcome::Enqueued);
  let desc: &str = (&outcome).into();
  assert_eq!(desc, "enqueue");
}

#[test]
fn offer_outcome_dropped_oldest_variant() {
  let outcome = OfferOutcome::DroppedOldest { count: 3 };
  if let OfferOutcome::DroppedOldest { count } = outcome {
    assert_eq!(count, 3);
  } else {
    panic!("Expected DroppedOldest variant");
  }
  let desc: &str = (&outcome).into();
  assert_eq!(desc, "drop_oldest");
}

#[test]
fn offer_outcome_dropped_newest_variant() {
  let outcome = OfferOutcome::DroppedNewest { count: 5 };
  if let OfferOutcome::DroppedNewest { count } = outcome {
    assert_eq!(count, 5);
  } else {
    panic!("Expected DroppedNewest variant");
  }
  let desc: &str = (&outcome).into();
  assert_eq!(desc, "drop_newest");
}

#[test]
fn offer_outcome_grew_to_variant() {
  let outcome = OfferOutcome::GrewTo { capacity: 100 };
  if let OfferOutcome::GrewTo { capacity } = outcome {
    assert_eq!(capacity, 100);
  } else {
    panic!("Expected GrewTo variant");
  }
  let desc: &str = (&outcome).into();
  assert_eq!(desc, "grow");
}

#[test]
fn offer_outcome_clone_works() {
  let original = OfferOutcome::DroppedOldest { count: 2 };
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn offer_outcome_copy_works() {
  let original = OfferOutcome::Enqueued;
  let copied = original;
  assert_eq!(original, copied);
}

#[test]
fn offer_outcome_debug_format() {
  let outcome = OfferOutcome::GrewTo { capacity: 50 };
  let debug_str = format!("{:?}", outcome);
  assert!(debug_str.contains("GrewTo"));
  assert!(debug_str.contains("50"));
}

#[test]
fn offer_outcome_partial_eq() {
  assert_eq!(OfferOutcome::Enqueued, OfferOutcome::Enqueued);
  assert_ne!(OfferOutcome::Enqueued, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(OfferOutcome::DroppedOldest { count: 2 }, OfferOutcome::DroppedOldest { count: 2 });
  assert_ne!(OfferOutcome::DroppedOldest { count: 2 }, OfferOutcome::DroppedOldest { count: 3 });
}
