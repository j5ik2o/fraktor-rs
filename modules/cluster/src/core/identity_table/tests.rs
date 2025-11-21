use alloc::string::ToString;

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPathFormatter;

use crate::core::{
  identity_event::IdentityEvent,
  identity_table::IdentityTable,
  membership_delta::MembershipDelta,
  membership_version::MembershipVersion,
  node_record::NodeRecord,
  node_status::NodeStatus,
  resolve_error::ResolveError,
  resolve_result::ResolveResult,
};

#[test]
fn ready_returns_canonical_path_and_latest_version() {
  let mut membership = crate::core::membership_table::MembershipTable::new(2);
  membership.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  membership.drain_events();

  let mut table = IdentityTable::new(membership);

  let result = table.resolve("n1:4050", "user/echo").expect("resolve should succeed");

  match result {
    | ResolveResult::Ready { actor_path, version } => {
      assert_eq!(version, MembershipVersion::new(1));
      let uri = ActorPathFormatter::format(&actor_path);
      assert_eq!(uri, "fraktor.tcp://cellactor@n1:4050/user/echo");
    },
    | other => panic!("unexpected result: {other:?}"),
  }

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![IdentityEvent::ResolvedLatest { authority: "n1:4050".to_string(), version: MembershipVersion::new(1) }],
  );
}

#[test]
fn unreachable_is_returned_for_removed_node() {
  let mut membership = crate::core::membership_table::MembershipTable::new(2);
  membership.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  membership.drain_events();
  membership.mark_left("n1:4050").expect("leave succeeds");

  let mut table = IdentityTable::new(membership);

  let result = table.resolve("n1:4050", "user/echo").expect("resolve should succeed");

  match result {
    | ResolveResult::Unreachable { authority, version } => {
      assert_eq!(authority, "n1:4050");
      assert_eq!(version, MembershipVersion::new(2));
    },
    | other => panic!("unexpected result: {other:?}"),
  }
}

#[test]
fn unreachable_is_returned_for_missing_authority() {
  let membership = crate::core::membership_table::MembershipTable::new(2);
  let mut table = IdentityTable::new(membership);

  let result = table.resolve("n1:4050", "user/echo").expect("resolve should succeed");

  match result {
    | ResolveResult::Unreachable { authority, version } => {
      assert_eq!(authority, "n1:4050");
      assert_eq!(version, MembershipVersion::zero());
    },
    | other => panic!("unexpected result: {other:?}"),
  }

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![IdentityEvent::UnknownAuthority { authority: "n1:4050".to_string(), version: MembershipVersion::zero() }],
  );
}

#[test]
fn quarantine_takes_precedence_over_membership() {
  let mut membership = crate::core::membership_table::MembershipTable::new(2);
  membership.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  membership.drain_events();

  let mut table = IdentityTable::new(membership);
  table.quarantine("n1:4050".to_string(), "manual".to_string());

  let result = table.resolve("n1:4050", "user/echo").expect("resolve should succeed");

  match result {
    | ResolveResult::Quarantine { authority, reason, version } => {
      assert_eq!(authority, "n1:4050");
      assert_eq!(reason, "manual");
      assert_eq!(version, MembershipVersion::new(1));
    },
    | other => panic!("unexpected result: {other:?}"),
  }

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![IdentityEvent::Quarantined {
      authority: "n1:4050".to_string(),
      reason: "manual".to_string(),
      version: MembershipVersion::new(1),
    }],
  );
}

#[test]
fn invalid_format_returns_error_and_event() {
  let membership = crate::core::membership_table::MembershipTable::new(2);
  let mut table = IdentityTable::new(membership);

  let err = table
    .resolve("n1", "user/echo")
    .expect_err("missing port should be invalid");

  assert_eq!(err, ResolveError::InvalidFormat { reason: "authority missing port".to_string() });

  let events = table.drain_events();
  assert_eq!(events, vec![IdentityEvent::InvalidFormat { reason: "authority missing port".to_string() }]);
}

#[test]
fn resolve_uses_latest_version_after_delta() {
  let mut membership = crate::core::membership_table::MembershipTable::new(2);
  membership.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  membership.drain_events();

  let mut table = IdentityTable::new(membership);

  // 追加 delta を適用
  let delta = MembershipDelta::new(
    MembershipVersion::new(1),
    MembershipVersion::new(2),
    vec![NodeRecord::new(
      "node-2".to_string(),
      "n2:4051".to_string(),
      NodeStatus::Up,
      MembershipVersion::new(2),
    )],
  );
  table.apply_membership_delta(delta);

  let result = table.resolve("n2:4051", "user/echo").expect("resolve should succeed");

  match result {
    | ResolveResult::Ready { version, .. } => assert_eq!(version, MembershipVersion::new(2)),
    | other => panic!("unexpected result: {other:?}"),
  }

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![IdentityEvent::ResolvedLatest { authority: "n2:4051".to_string(), version: MembershipVersion::new(2) }],
  );
}
