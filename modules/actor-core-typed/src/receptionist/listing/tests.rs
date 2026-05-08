use alloc::{collections::BTreeSet, vec, vec::Vec};
use core::any::TypeId;

use crate::receptionist::{Listing, ServiceKey};

#[test]
fn listing_should_store_fields() {
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);
  assert_eq!(listing.service_id(), "svc");
  assert_eq!(listing.type_id(), TypeId::of::<u32>());
  assert!(listing.is_empty());
}

#[test]
fn typed_refs_should_fail_when_type_id_mismatches() {
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let mismatch = listing.typed_refs::<u64>();
  assert!(mismatch.is_err());
}

#[test]
fn typed_refs_should_succeed_when_type_id_matches() {
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let refs = listing.typed_refs::<u32>().expect("matching type should succeed");
  assert!(refs.is_empty());
}

// --- Phase 1 タスク4: is_for_key / service_instances ---

/// `is_for_key` returns `true` when both service_id and type_id match.
#[test]
fn is_for_key_returns_true_for_matching_key() {
  let key = ServiceKey::<u32>::new("my-service");
  let listing = Listing::new("my-service", TypeId::of::<u32>(), vec![]);

  assert!(listing.is_for_key(&key), "should match when service_id and type_id are equal");
}

/// `is_for_key` returns `false` when service_id differs.
#[test]
fn is_for_key_returns_false_for_different_service_id() {
  let key = ServiceKey::<u32>::new("other-service");
  let listing = Listing::new("my-service", TypeId::of::<u32>(), vec![]);

  assert!(!listing.is_for_key(&key), "should not match when service_id differs");
}

/// `is_for_key` returns `false` when type_id differs.
#[test]
fn is_for_key_returns_false_for_different_type_id() {
  let key = ServiceKey::<u64>::new("my-service");
  let listing = Listing::new("my-service", TypeId::of::<u32>(), vec![]);

  assert!(!listing.is_for_key(&key), "should not match when type_id differs");
}

/// `is_for_key` returns `false` when both service_id and type_id differ.
#[test]
fn is_for_key_returns_false_when_both_differ() {
  let key = ServiceKey::<u64>::new("other-service");
  let listing = Listing::new("my-service", TypeId::of::<u32>(), vec![]);

  assert!(!listing.is_for_key(&key), "should not match when both differ");
}

/// `service_instances` returns typed refs for a matching key.
#[test]
fn service_instances_returns_refs_for_matching_key() {
  let key = ServiceKey::<u32>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let instances: BTreeSet<_> = listing.service_instances(&key).expect("should succeed for matching key");
  assert!(instances.is_empty(), "empty listing should return an empty set");
}

/// `all_service_instances` returns typed refs for a matching key.
#[test]
fn all_service_instances_returns_refs_for_matching_key() {
  let key = ServiceKey::<u32>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let instances: BTreeSet<_> = listing.all_service_instances(&key).expect("should succeed for matching key");
  assert!(instances.is_empty(), "empty listing should return an empty set");
}

/// `service_instances` returns error for a mismatched key.
#[test]
fn service_instances_returns_error_for_mismatched_key() {
  let key = ServiceKey::<u64>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let result = listing.service_instances(&key);
  assert!(result.is_err(), "should fail when type_id does not match");
}

/// `all_service_instances` returns error for a mismatched key.
#[test]
fn all_service_instances_returns_error_for_mismatched_key() {
  let key = ServiceKey::<u64>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let result = listing.all_service_instances(&key);
  assert!(result.is_err(), "should fail when type_id does not match");
}

/// `service_instances` returns error when service_id does not match.
#[test]
fn service_instances_returns_error_for_mismatched_service_id() {
  let key = ServiceKey::<u32>::new("other");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);

  let result = listing.service_instances(&key);
  assert!(result.is_err(), "should fail when service_id does not match");
}

/// `service_instances` with actual actor refs returns correctly typed refs.
#[test]
fn service_instances_with_refs_returns_typed_refs() {
  use fraktor_actor_core_rs::core::kernel::actor::{
    Pid,
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  };

  struct StubSender;
  impl ActorRefSender for StubSender {
    fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
      Ok(SendOutcome::Delivered)
    }
  }

  let refs = vec![
    crate::test_support::actor_ref_with_sender(Pid::new(1, 0), StubSender),
    crate::test_support::actor_ref_with_sender(Pid::new(2, 0), StubSender),
  ];
  let key = ServiceKey::<u32>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), refs);

  let instances = listing.service_instances(&key).expect("should succeed");
  assert_eq!(instances.len(), 2, "should return all registered refs");
  let pids = instances.into_iter().map(|actor_ref| actor_ref.pid()).collect::<Vec<_>>();
  assert_eq!(pids, vec![Pid::new(1, 0), Pid::new(2, 0)]);
}

/// `service_instances` deduplicates repeated actor refs into a set contract.
#[test]
fn service_instances_deduplicate_duplicate_refs() {
  use fraktor_actor_core_rs::core::kernel::actor::{
    Pid,
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  };

  struct StubSender;
  impl ActorRefSender for StubSender {
    fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
      Ok(SendOutcome::Delivered)
    }
  }

  let duplicate = crate::test_support::actor_ref_with_sender(Pid::new(9, 0), StubSender);
  let key = ServiceKey::<u32>::new("svc");
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![duplicate.clone(), duplicate]);

  let instances = listing.service_instances(&key).expect("should succeed");
  assert_eq!(instances.len(), 1, "service_instances should expose a set contract");
  assert_eq!(instances.into_iter().next().expect("one instance").pid(), Pid::new(9, 0));
}

/// `services_were_added_or_removed` is always `true` for local-only listings.
#[test]
fn services_were_added_or_removed_returns_true_for_local_listing() {
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);
  assert!(listing.services_were_added_or_removed());
}
