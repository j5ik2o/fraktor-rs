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

fn has_cache_drop_event(events: &[PidCacheEvent], key: &GrainKey) -> bool {
  events.iter().any(|event| matches!(event, PidCacheEvent::Dropped { key: event_key, .. } if event_key == key))
}

const FAR_FUTURE_LEASE_EXPIRES_AT: u64 = u64::MAX;

fn clear_observed_events(lookup: &mut PartitionIdentityLookup) {
  drop(lookup.drain_events());
  drop(lookup.drain_cache_events());
}

const fn command_request_id(command: &PlacementCommand) -> PlacementRequestId {
  match command {
    | PlacementCommand::TryAcquire { request_id, .. }
    | PlacementCommand::LoadActivation { request_id, .. }
    | PlacementCommand::EnsureActivation { request_id, .. }
    | PlacementCommand::StoreActivation { request_id, .. }
    | PlacementCommand::Release { request_id, .. } => *request_id,
  }
}

fn only_command<'a>(commands: &'a [PlacementCommand], label: &str) -> &'a PlacementCommand {
  assert_eq!(commands.len(), 1, "{label} should emit exactly one command, got {commands:?}");
  &commands[0]
}

fn begin_pending_activation(
  lookup: &mut PartitionIdentityLookup,
  key: &GrainKey,
  now: u64,
) -> (PlacementRequestId, String) {
  let outcome = lookup.resolve_outcome(key, now).expect("pending activation should resolve to command outcome");
  assert!(outcome.resolution.is_none(), "distributed activation should be pending until commands complete");
  let command = only_command(&outcome.commands, "try-acquire command");
  let PlacementCommand::TryAcquire { request_id, key: command_key, owner, now: command_now } = command else {
    panic!("expected TryAcquire for pending activation, got {command:?}");
  };
  assert_eq!(command_key, key);
  assert_eq!(*command_now, now);
  (*request_id, owner.clone())
}

/// Completes a pending activation after the caller has driven `resolve`.
///
/// The caller must pass the request id that the coordinator emitted for that
/// pending activation. Once the first command result is accepted, this helper
/// propagates request ids from each emitted command into the next result.
fn complete_pending_activation(
  lookup: &mut PartitionIdentityLookup,
  request_id: PlacementRequestId,
  key: &GrainKey,
  owner: &str,
  pid: &str,
) -> PlacementResolution {
  // Keep the protocol steps expanded so the contract names each emitted command
  // transition instead of hiding the placement flow behind a loop table.
  let lease =
    PlacementLease { key: key.clone(), owner: owner.to_string(), expires_at: FAR_FUTURE_LEASE_EXPIRES_AT };

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockAcquired { request_id, result: Ok(lease) })
    .unwrap_or_else(|err| panic!("LockAcquired for {request_id:?} should produce LoadActivation: {err:?}"));
  let command = only_command(&outcome.commands, "load command");
  assert!(
    matches!(command, PlacementCommand::LoadActivation { .. }),
    "expected LoadActivation after LockAcquired, got {command:?}"
  );
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationLoaded { request_id, result: Ok(None) })
    .unwrap_or_else(|err| panic!("ActivationLoaded for {request_id:?} should produce EnsureActivation: {err:?}"));
  let command = only_command(&outcome.commands, "ensure command");
  assert!(
    matches!(command, PlacementCommand::EnsureActivation { .. }),
    "expected EnsureActivation after ActivationLoaded, got {command:?}",
  );
  let request_id = command_request_id(command);

  let record = ActivationRecord::new(pid.to_string(), None, 0);
  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationEnsured { request_id, result: Ok(record.clone()) })
    .unwrap_or_else(|err| panic!("ActivationEnsured for {request_id:?} should produce StoreActivation: {err:?}"));
  let command = only_command(&outcome.commands, "store command");
  assert!(
    matches!(command, PlacementCommand::StoreActivation { .. }),
    "expected StoreActivation after ActivationEnsured, got {command:?}",
  );
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationStored { request_id, result: Ok(()) })
    .unwrap_or_else(|err| panic!("ActivationStored for {request_id:?} should produce Release: {err:?}"));
  let command = only_command(&outcome.commands, "release command");
  assert!(
    matches!(command, PlacementCommand::Release { .. }),
    "expected Release after ActivationStored, got {command:?}"
  );
  let request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockReleased { request_id, result: Ok(()) })
    .unwrap_or_else(|err| panic!("LockReleased for {request_id:?} should complete activation: {err:?}"));
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
  lookup.set_local_authority("node-a:4050");
  lookup.set_distributed_activation(true);
  let key = grain_key("user/cache-hit");

  let (request_id, owner) = begin_pending_activation(&mut lookup, &key, 1000);
  let first = complete_pending_activation(&mut lookup, request_id, &key, &owner, "custom-cache-hit-pid");
  clear_observed_events(&mut lookup);
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

  let (request_id, owner) = begin_pending_activation(&mut lookup, &key, 1000);
  let pid = "custom-pending-pid";
  let resolution = complete_pending_activation(&mut lookup, request_id, &key, &owner, pid);
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
  clear_observed_events(&mut lookup);

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
  let (request_id, owner) = begin_pending_activation(&mut lookup, &key, 1000);
  // The PID string is intentionally unrelated to the authority. Member-left
  // invalidation must be driven by the lease owner, not by parsing the PID.
  let first = complete_pending_activation(&mut lookup, request_id, &key, &owner, "custom-member-left-pid");
  clear_observed_events(&mut lookup);

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
  assert!(matches!(after_departure, Err(LookupError::Pending)));
}

#[test]
fn passivation_removes_idle_activation_but_keeps_recent_activation() {
  const IDLE_ACTIVATED_AT: u64 = 1000;
  const RECENT_ACTIVATED_AT: u64 = 1150;
  const PASSIVATION_NOW: u64 = 1200;
  const IDLE_TTL: u64 = 100;

  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  lookup.set_local_authority("node-a:4050");
  lookup.set_distributed_activation(true);
  let recent_key = grain_key("user/recent");
  let idle_key = grain_key("user/idle");
  let (idle_request_id, idle_owner) = begin_pending_activation(&mut lookup, &idle_key, IDLE_ACTIVATED_AT);
  let _idle = complete_pending_activation(&mut lookup, idle_request_id, &idle_key, &idle_owner, "custom-idle-pid");
  clear_observed_events(&mut lookup);
  let (recent_request_id, recent_owner) = begin_pending_activation(&mut lookup, &recent_key, RECENT_ACTIVATED_AT);
  let recent =
    complete_pending_activation(&mut lookup, recent_request_id, &recent_key, &recent_owner, "custom-recent-pid");
  clear_observed_events(&mut lookup);

  lookup.passivate_idle(PASSIVATION_NOW, IDLE_TTL);
  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&placement_events, &idle_key));
  assert!(!has_passivated_event(&placement_events, &recent_key));
  assert!(has_cache_drop_event(&cache_events, &idle_key));
  assert!(!has_cache_drop_event(&cache_events, &recent_key));

  let recent_again = lookup.resolve(&recent_key, PASSIVATION_NOW + 1).expect("recent cached");
  assert_eq!(recent_again.pid, recent.pid);

  let idle_after_passivation = lookup.resolve(&idle_key, PASSIVATION_NOW + 1);
  assert!(matches!(idle_after_passivation, Err(LookupError::Pending)));
}

#[test]
fn rolling_update_prevents_stale_authority_reuse_without_rebalance_guarantees() {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["old-node:4050".to_string()]);
  let key = grain_key("user/rolling-update");
  let old = lookup.resolve(&key, 1000).expect("old resolution");
  clear_observed_events(&mut lookup);

  lookup.update_topology(vec!["old-node:4050".to_string(), "new-node:4051".to_string()]);
  let mixed_placement_events = lookup.drain_events();
  let mixed_cache_events = lookup.drain_cache_events();
  assert!(!has_passivated_event(&mixed_placement_events, &key));
  assert!(!has_cache_drop_event(&mixed_cache_events, &key));

  lookup.on_member_left("old-node:4050");
  let left_placement_events = lookup.drain_events();
  let left_cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&left_placement_events, &key));
  assert!(has_cache_drop_event(&left_cache_events, &key));

  lookup.update_topology(vec!["new-node:4051".to_string()]);
  let final_placement_events = lookup.drain_events();
  let final_cache_events = lookup.drain_cache_events();
  assert!(!has_passivated_event(&final_placement_events, &key));
  assert!(!has_cache_drop_event(&final_cache_events, &key));

  let updated = lookup.resolve(&key, 1001).expect("updated resolution");
  assert_eq!(updated.decision.authority, "new-node:4051");
  assert_ne!(updated.decision.authority, old.decision.authority);
  assert_ne!(updated.pid, old.pid);
  // This contract is intentionally bounded to stale authority invalidation and
  // re-resolution. Rebalance, remembered entity recovery, and request draining
  // belong to later changes.
}
