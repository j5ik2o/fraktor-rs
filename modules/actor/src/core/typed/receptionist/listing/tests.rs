use alloc::vec;
use core::any::TypeId;

use crate::core::typed::receptionist::Listing;

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
