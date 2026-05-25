use fraktor_cluster_core_rs::{
  grain::GrainKey,
  identity::{IdentityLookup, LookupError, PartitionIdentityLookup, PidCacheEvent},
  placement::{
    ActivationRecord, PlacementCommand, PlacementCommandResult, PlacementEvent, PlacementLease, PlacementLocality,
    PlacementRequestId, PlacementResolution,
  },
};

fn member_lookup() -> PartitionIdentityLookup {
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.setup_member(&[]).expect("setup member");
  lookup
}

fn grain_key(value: &str) -> GrainKey {
  GrainKey::new(value.to_string())
}

fn has_passivated_event(events: &[PlacementEvent], key: &GrainKey) -> bool {
  events.iter().any(|event| matches!(event, PlacementEvent::Passivated { key: event_key, .. } if event_key == key))
}

fn has_activated_event(events: &[PlacementEvent], key: &GrainKey) -> bool {
  events.iter().any(|event| matches!(event, PlacementEvent::Activated { key: event_key, .. } if event_key == key))
}

fn has_cache_drop_event(events: &[PidCacheEvent], key: &GrainKey) -> bool {
  events.iter().any(|event| matches!(event, PidCacheEvent::Dropped { key: event_key, .. } if event_key == key))
}

const FAR_FUTURE_LEASE_EXPIRES_AT: u64 = 1300;

const fn command_request_id(command: &PlacementCommand) -> PlacementRequestId {
  match command {
    | PlacementCommand::TryAcquire { request_id, .. }
    | PlacementCommand::LoadActivation { request_id, .. }
    | PlacementCommand::EnsureActivation { request_id, .. }
    | PlacementCommand::StoreActivation { request_id, .. }
    | PlacementCommand::Release { request_id, .. } => *request_id,
  }
}

/// Completes the first pending activation created by `PartitionIdentityLookup::resolve`.
///
/// `PartitionIdentityLookup` currently reports only `LookupError::Pending`, so
/// the initial `request_id` is the coordinator's first issued request id.
fn complete_pending_activation(
  lookup: &mut PartitionIdentityLookup,
  request_id: PlacementRequestId,
  key: &GrainKey,
  pid: &str,
) -> PlacementResolution {
  let lease = PlacementLease {
    key:        key.clone(),
    owner:      "node-a:4050".to_string(),
    expires_at: FAR_FUTURE_LEASE_EXPIRES_AT,
  };

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockAcquired { request_id, result: Ok(lease) })
    .expect("lock acquired");
  let command = outcome.commands.first().expect("load command");
  assert!(matches!(command, PlacementCommand::LoadActivation { .. }));
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationLoaded { request_id, result: Ok(None) })
    .expect("activation loaded");
  let command = outcome.commands.first().expect("ensure command");
  assert!(matches!(command, PlacementCommand::EnsureActivation { .. }));
  let request_id = command_request_id(command);

  let record = ActivationRecord::new(pid.to_string(), None, 0);
  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationEnsured { request_id, result: Ok(record.clone()) })
    .expect("activation ensured");
  let command = outcome.commands.first().expect("store command");
  assert!(matches!(command, PlacementCommand::StoreActivation { .. }));
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationStored { request_id, result: Ok(()) })
    .expect("activation stored");
  let command = outcome.commands.first().expect("release command");
  assert!(matches!(command, PlacementCommand::Release { .. }));
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockReleased { request_id, result: Ok(()) })
    .expect("lock released");
  outcome.resolution.expect("completed resolution")
}

#[test]
fn no_authority_resolution_fails_without_cache_side_effects() {
  let mut lookup = member_lookup();
  let key = grain_key("user/no-authority");

  let result = lookup.resolve(&key, 1000);

  assert!(matches!(result, Err(LookupError::NoAuthority)));
  assert!(lookup.drain_events().is_empty());
  assert!(lookup.drain_cache_events().is_empty());
}

#[test]
fn same_key_and_topology_resolve_to_the_same_authority() {
  let authorities = vec!["node-a:4050".to_string(), "node-b:4051".to_string(), "node-c:4052".to_string()];
  let mut first_lookup = member_lookup();
  let mut second_lookup = member_lookup();
  first_lookup.update_topology(authorities.clone());
  second_lookup.update_topology(authorities);
  let key = grain_key("user/deterministic");

  let first = first_lookup.resolve(&key, 1000).expect("first resolution");
  let second = second_lookup.resolve(&key, 1000).expect("second resolution");

  assert_eq!(first.decision.authority, second.decision.authority);
  assert_eq!(first.decision.key, key);
  assert_eq!(second.decision.key, key);
}

#[test]
fn active_pid_is_reused_until_cache_or_passivation_invalidates_it() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  let key = grain_key("user/cache-hit");

  let first = lookup.resolve(&key, 1000).expect("first resolution");
  let _ = lookup.drain_events();
  let second = lookup.resolve(&key, 1001).expect("second resolution");

  assert_eq!(first.pid, second.pid);
  assert_eq!(first.decision.authority, second.decision.authority);
  assert!(lookup.drain_cache_events().is_empty());
}

#[test]
fn distributed_activation_reports_pending_then_completes_after_command_results() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  lookup.set_local_authority("node-a:4050");
  lookup.set_distributed_activation(true);
  let key = grain_key("user/pending");

  let pending = lookup.resolve(&key, 1000);
  assert!(matches!(pending, Err(LookupError::Pending)));

  let pid = "node-a:4050::user/pending";
  let resolution = complete_pending_activation(&mut lookup, PlacementRequestId(1), &key, pid);
  assert_eq!(resolution.pid, pid);
  assert_eq!(resolution.locality, PlacementLocality::Local);

  let completed = lookup.resolve(&key, 1001).expect("completed lookup resolution");
  assert_eq!(completed.pid, pid);
}

#[test]
fn topology_replacement_invalidates_absent_authority_cache_and_reresolves() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  let key = grain_key("user/topology-replacement");
  let original = lookup.resolve(&key, 1000).expect("original resolution");
  let _ = lookup.drain_events();
  let _ = lookup.drain_cache_events();

  lookup.update_topology(vec!["node-b:4051".to_string()]);

  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&placement_events, &key));
  assert!(has_cache_drop_event(&cache_events, &key));

  let updated = lookup.resolve(&key, 1001).expect("updated resolution");
  assert_eq!(updated.decision.authority, "node-b:4051");
  assert_ne!(updated.decision.authority, original.decision.authority);
  assert_ne!(updated.pid, original.pid);
}

#[test]
fn member_departure_invalidates_matching_authority_but_unknown_departure_is_noop() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  lookup.set_local_authority("node-a:4050");
  lookup.set_distributed_activation(true);
  let key = grain_key("user/member-left");
  let pending = lookup.resolve(&key, 1000);
  assert!(matches!(pending, Err(LookupError::Pending)));
  // The PID string is intentionally unrelated to the authority. Member-left
  // invalidation must be driven by the lease owner, not by parsing the PID.
  let first = complete_pending_activation(&mut lookup, PlacementRequestId(1), &key, "custom-member-left-pid");
  let _ = lookup.drain_events();
  let _ = lookup.drain_cache_events();

  lookup.on_member_left("node-z:4099");
  assert!(lookup.drain_events().is_empty());
  assert!(lookup.drain_cache_events().is_empty());
  let still_cached = lookup.resolve(&key, 1001).expect("cached after unknown departure");
  assert_eq!(still_cached.pid, first.pid);

  lookup.on_member_left("node-a:4050");
  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();

  assert!(has_passivated_event(&placement_events, &key));
  assert!(has_cache_drop_event(&cache_events, &key));

  let after_departure = lookup.resolve(&key, 1002);
  assert!(!matches!(after_departure, Ok(resolution) if resolution.pid == first.pid));
}

#[test]
fn passivation_removes_idle_activation_but_keeps_recent_activation() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  let recent_key = grain_key("user/recent");
  let idle_key = grain_key("user/idle");
  let idle = lookup.resolve(&idle_key, 1000).expect("idle resolution");
  let _ = lookup.drain_events();
  let _ = lookup.drain_cache_events();
  let recent = lookup.resolve(&recent_key, 1150).expect("recent resolution");
  let _ = lookup.drain_events();
  let _ = lookup.drain_cache_events();

  lookup.passivate_idle(1200, 100);
  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&placement_events, &idle_key));
  assert!(!has_passivated_event(&placement_events, &recent_key));
  assert!(has_cache_drop_event(&cache_events, &idle_key));
  assert!(!has_cache_drop_event(&cache_events, &recent_key));

  let recent_again = lookup.resolve(&recent_key, 1201).expect("recent cached");
  assert_eq!(recent_again.pid, recent.pid);

  let idle_again = lookup.resolve(&idle_key, 1201).expect("idle reactivated");
  assert_eq!(idle_again.decision.authority, idle.decision.authority);
  assert!(has_activated_event(&lookup.drain_events(), &idle_key));
}

#[test]
fn rolling_update_prevents_stale_authority_reuse_without_rebalance_guarantees() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["old-node:4050".to_string()]);
  let key = grain_key("user/rolling-update");
  let old = lookup.resolve(&key, 1000).expect("old resolution");
  let _ = lookup.drain_events();
  let _ = lookup.drain_cache_events();

  lookup.update_topology(vec!["old-node:4050".to_string(), "new-node:4051".to_string()]);
  lookup.on_member_left("old-node:4050");
  lookup.update_topology(vec!["new-node:4051".to_string()]);

  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&placement_events, &key));
  assert!(has_cache_drop_event(&cache_events, &key));

  let updated = lookup.resolve(&key, 1001).expect("updated resolution");
  assert_eq!(updated.decision.authority, "new-node:4051");
  assert_ne!(updated.decision.authority, old.decision.authority);
  assert_ne!(updated.pid, old.pid);
  // This contract is intentionally bounded to stale authority invalidation and
  // re-resolution. Rebalance, remembered entity recovery, and request draining
  // belong to later changes.
}
