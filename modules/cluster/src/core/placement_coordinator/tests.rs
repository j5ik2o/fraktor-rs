use alloc::vec;

use crate::core::{
  activation_record::ActivationRecord, grain_key::GrainKey, placement_command::PlacementCommand,
  placement_command_result::PlacementCommandResult, placement_coordinator::PlacementCoordinatorCore,
  placement_coordinator_error::PlacementCoordinatorError, placement_lease::PlacementLease,
  placement_locality::PlacementLocality, placement_request_id::PlacementRequestId, rendezvous_hasher::RendezvousHasher,
};

#[test]
fn resolve_returns_remote_for_remote_owner() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");

  let authorities = vec!["node1:8080".to_string(), "node2:8080".to_string()];
  let key = GrainKey::new("user/remote".to_string());
  let owner = RendezvousHasher::select(&authorities, &key).expect("owner");
  let local = if owner == "node1:8080" { "node2:8080" } else { "node1:8080" };

  coordinator.update_topology(authorities);
  coordinator.set_local_authority(local.to_string());

  let outcome = coordinator.resolve(&key, 1000).expect("resolve");
  let resolution = outcome.resolution.expect("resolution");
  assert_eq!(resolution.locality, PlacementLocality::Remote);
}

#[test]
fn resolve_generates_command_when_distributed_activation_enabled() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");
  coordinator.set_distributed_activation(true);
  coordinator.update_topology(vec!["node1:8080".to_string()]);
  coordinator.set_local_authority("node1:8080".to_string());

  let key = GrainKey::new("user/local".to_string());
  let outcome = coordinator.resolve(&key, 1000).expect("resolve");

  assert!(outcome.resolution.is_none());
  assert_eq!(outcome.commands.len(), 1);
  assert!(matches!(outcome.commands[0], PlacementCommand::TryAcquire { .. }));
}

#[test]
fn handle_command_result_rejects_unknown_request() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");
  coordinator.update_topology(vec!["node1:8080".to_string()]);

  let result = PlacementCommandResult::LockAcquired {
    request_id: PlacementRequestId(999),
    result:     Err(crate::core::placement_lock_error::PlacementLockError::Failed { reason: "missing".to_string() }),
  };

  let err = coordinator.handle_command_result(result).expect_err("unknown request");
  assert!(matches!(err, PlacementCoordinatorError::UnknownRequest { .. }));
}

#[test]
fn handle_command_result_completes_activation_flow() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");
  coordinator.set_distributed_activation(true);
  coordinator.update_topology(vec!["node1:8080".to_string()]);
  coordinator.set_local_authority("node1:8080".to_string());

  let key = GrainKey::new("user/flow".to_string());
  let outcome = coordinator.resolve(&key, 1000).expect("resolve");
  let command = outcome.commands.first().expect("command").clone();
  let PlacementCommand::TryAcquire { request_id, .. } = command else {
    panic!("expected TryAcquire");
  };

  let lease = PlacementLease { key: key.clone(), owner: "node1:8080".to_string(), expires_at: 2000 };
  let outcome = coordinator
    .handle_command_result(PlacementCommandResult::LockAcquired { request_id, result: Ok(lease.clone()) })
    .expect("lock acquired");
  let command = outcome.commands.first().expect("load command").clone();
  assert!(matches!(command, PlacementCommand::LoadActivation { .. }));

  let outcome = coordinator
    .handle_command_result(PlacementCommandResult::ActivationLoaded { request_id, result: Ok(None) })
    .expect("activation loaded");
  let command = outcome.commands.first().expect("ensure command").clone();
  assert!(matches!(command, PlacementCommand::EnsureActivation { .. }));

  let record = ActivationRecord::new("node1:8080::user/flow".to_string(), None, 0);
  let outcome = coordinator
    .handle_command_result(PlacementCommandResult::ActivationEnsured { request_id, result: Ok(record.clone()) })
    .expect("activation ensured");
  let command = outcome.commands.first().expect("store command").clone();
  assert!(matches!(command, PlacementCommand::StoreActivation { .. }));

  let outcome = coordinator
    .handle_command_result(PlacementCommandResult::ActivationStored { request_id, result: Ok(()) })
    .expect("activation stored");
  let command = outcome.commands.first().expect("release command").clone();
  assert!(matches!(command, PlacementCommand::Release { .. }));

  let outcome = coordinator
    .handle_command_result(PlacementCommandResult::LockReleased { request_id, result: Ok(()) })
    .expect("lock released");
  let resolution = outcome.resolution.expect("resolution");
  assert_eq!(resolution.pid, record.pid);
}
