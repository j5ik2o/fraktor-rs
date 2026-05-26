use alloc::{format, string::ToString};

use crate::{
  grain::GrainKey,
  identity::{IdentityLookup, LookupError, PartitionIdentityLookup, PidCacheEvent, RendezvousHasher},
  placement::{
    ActivationRecord, PlacementCommand, PlacementCommandResult, PlacementCoordinatorOutcome, PlacementEvent,
    PlacementLease, PlacementLocality, PlacementRequestId, PlacementResolution,
  },
};

impl PartitionIdentityLookup {
  // contract test が pending placement flow を駆動できるように、テスト内だけで
  // coordinator の raw outcome を観測する。
  fn resolve_outcome(&mut self, key: &GrainKey, now: u64) -> Result<PlacementCoordinatorOutcome, LookupError> {
    self.coordinator.resolve(key, now)
  }
}

fn member_lookup() -> PartitionIdentityLookup {
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.setup_member(&[]).expect("setup member");
  lookup
}

fn grain_key(value: &str) -> GrainKey {
  GrainKey::new(value.to_string())
}

fn key_owned_by(authorities: &[String], owner: &str, prefix: &str) -> GrainKey {
  for index in 0..10_000 {
    let key = grain_key(&format!("{prefix}/{index}"));
    if let Some(selected) = RendezvousHasher::select(authorities, &key)
      && selected == owner
    {
      return key;
    }
  }
  panic!("no key owned by {owner} found for {prefix}");
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
  try_acquire_request_id: PlacementRequestId,
  key: &GrainKey,
  owner: &str,
  pid: &str,
) -> PlacementResolution {
  // protocol の各 step を展開しておくことで、placement の流れを loop table に
  // 隠さず、契約として emitted command transition を明示する。
  let lease =
    PlacementLease { key: key.clone(), owner: owner.to_string(), expires_at: FAR_FUTURE_LEASE_EXPIRES_AT };

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockAcquired {
      request_id: try_acquire_request_id,
      result:     Ok(lease),
    })
    .unwrap_or_else(|err| panic!("LockAcquired for {try_acquire_request_id:?} should produce LoadActivation: {err:?}"));
  let command = only_command(&outcome.commands, "load command");
  assert!(
    matches!(command, PlacementCommand::LoadActivation { .. }),
    "expected LoadActivation after LockAcquired, got {command:?}"
  );
  let load_request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationLoaded {
      request_id: load_request_id,
      result:     Ok(None),
    })
    .unwrap_or_else(|err| panic!("ActivationLoaded for {load_request_id:?} should produce EnsureActivation: {err:?}"));
  let command = only_command(&outcome.commands, "ensure command");
  assert!(
    matches!(command, PlacementCommand::EnsureActivation { .. }),
    "expected EnsureActivation after ActivationLoaded, got {command:?}",
  );
  let ensure_request_id = command_request_id(command);

  let record = ActivationRecord::new(pid.to_string(), None, 0);
  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationEnsured {
      request_id: ensure_request_id,
      result:     Ok(record.clone()),
    })
    .unwrap_or_else(|err| {
      panic!("ActivationEnsured for {ensure_request_id:?} should produce StoreActivation: {err:?}")
    });
  let command = only_command(&outcome.commands, "store command");
  assert!(
    matches!(command, PlacementCommand::StoreActivation { .. }),
    "expected StoreActivation after ActivationEnsured, got {command:?}",
  );
  let store_request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::ActivationStored {
      request_id: store_request_id,
      result:     Ok(()),
    })
    .unwrap_or_else(|err| panic!("ActivationStored for {store_request_id:?} should produce Release: {err:?}"));
  let command = only_command(&outcome.commands, "release command");
  assert!(
    matches!(command, PlacementCommand::Release { .. }),
    "expected Release after ActivationStored, got {command:?}"
  );
  let release_request_id = command_request_id(command);

  let outcome = lookup
    .handle_command_result(PlacementCommandResult::LockReleased { request_id: release_request_id, result: Ok(()) })
    .unwrap_or_else(|err| panic!("LockReleased for {release_request_id:?} should complete activation: {err:?}"));
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
fn active_pid_is_reused_on_repeated_resolve() {
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
fn join_does_not_move_existing_active_activation_when_rendezvous_owner_changes() {
  let original_topology = vec!["node-a:4050".to_string()];
  let expanded_topology = vec!["node-a:4050".to_string(), "node-b:4051".to_string()];
  let key = key_owned_by(&expanded_topology, "node-b:4051", "user/join-no-move");
  let mut lookup = member_lookup();
  lookup.update_topology(original_topology);

  let original = lookup.resolve(&key, 1000).expect("original resolution");
  assert_eq!(original.decision.authority, "node-a:4050");
  clear_observed_events(&mut lookup);

  lookup.update_topology(expanded_topology);
  let join_events = lookup.drain_events();
  let join_cache_events = lookup.drain_cache_events();
  assert!(!has_passivated_event(&join_events, &key));
  assert!(!has_cache_drop_event(&join_cache_events, &key));

  let after_join = lookup.resolve(&key, 1001).expect("cached resolution after join");
  assert_eq!(after_join.pid, original.pid);
  assert_eq!(after_join.decision.authority, original.decision.authority);
  assert!(lookup.drain_cache_events().is_empty());
}

#[test]
fn new_resolution_after_join_uses_expanded_topology_candidates() {
  let expanded_topology = vec!["node-a:4050".to_string(), "node-b:4051".to_string()];
  let key = key_owned_by(&expanded_topology, "node-b:4051", "user/join-new-resolution");
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);

  lookup.update_topology(expanded_topology);
  clear_observed_events(&mut lookup);

  let resolution = lookup.resolve(&key, 1000).expect("resolution after join");
  assert_eq!(resolution.decision.authority, "node-b:4051");
  assert!(lookup.authorities().iter().any(|authority| authority == &resolution.decision.authority));
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

fn member_left_lookup_with_active_entry() -> (PartitionIdentityLookup, GrainKey, PlacementResolution) {
  let mut lookup = member_lookup();
  lookup.update_topology(vec!["node-a:4050".to_string()]);
  lookup.set_local_authority("node-a:4050");
  lookup.set_distributed_activation(true);
  let key = grain_key("user/member-left");
  let (request_id, owner) = begin_pending_activation(&mut lookup, &key, 1000);
  // PID 文字列は authority と意図的に無関係にする。member-left invalidation は
  // PID の parse ではなく、lease owner によって駆動される必要がある。
  let first = complete_pending_activation(&mut lookup, request_id, &key, &owner, "custom-member-left-pid");
  clear_observed_events(&mut lookup);
  (lookup, key, first)
}

#[test]
fn member_departure_with_unknown_authority_is_noop_for_active_entries() {
  let (mut lookup, key, first) = member_left_lookup_with_active_entry();

  lookup.on_member_left("node-z:4099");
  assert!(lookup.drain_events().is_empty());
  assert!(lookup.drain_cache_events().is_empty());
  let still_cached = lookup.resolve(&key, 1001).expect("cached after unknown departure");
  assert_eq!(still_cached.pid, first.pid);
}

#[test]
fn member_departure_with_matching_authority_invalidates_active_entries_and_blocks_stale_pid_reuse() {
  let (mut lookup, key, _first) = member_left_lookup_with_active_entry();

  lookup.on_member_left("node-a:4050");
  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();

  assert!(has_passivated_event(&placement_events, &key));
  assert!(has_cache_drop_event(&cache_events, &key));

  let after_departure = lookup.resolve(&key, 1002);
  assert!(matches!(after_departure, Err(LookupError::Pending)));
}

#[test]
fn member_departure_invalidates_only_matching_authority_entries() {
  let authorities = vec!["node-a:4050".to_string(), "node-b:4051".to_string()];
  let node_a_key = key_owned_by(&authorities, "node-a:4050", "user/leave-node-a");
  let node_b_key = key_owned_by(&authorities, "node-b:4051", "user/leave-node-b");
  let mut lookup = member_lookup();
  lookup.update_topology(authorities);

  let node_a = lookup.resolve(&node_a_key, 1000).expect("node-a resolution");
  let node_b = lookup.resolve(&node_b_key, 1000).expect("node-b resolution");
  assert_eq!(node_a.decision.authority, "node-a:4050");
  assert_eq!(node_b.decision.authority, "node-b:4051");
  clear_observed_events(&mut lookup);

  lookup.on_member_left("node-a:4050");
  let placement_events = lookup.drain_events();
  let cache_events = lookup.drain_cache_events();
  assert!(has_passivated_event(&placement_events, &node_a_key));
  assert!(!has_passivated_event(&placement_events, &node_b_key));
  assert!(has_cache_drop_event(&cache_events, &node_a_key));
  assert!(!has_cache_drop_event(&cache_events, &node_b_key));

  let node_b_after_departure = lookup.resolve(&node_b_key, 1001).expect("remaining cached resolution");
  assert_eq!(node_b_after_departure.pid, node_b.pid);
  assert_eq!(node_b_after_departure.decision.authority, node_b.decision.authority);
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
  // この contract は stale authority invalidation と re-resolution に意図的に限定する。
  // rebalance、remembered entity recovery、request draining は後続 change で扱う。
}
