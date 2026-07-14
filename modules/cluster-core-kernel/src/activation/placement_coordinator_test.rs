use alloc::vec;

use crate::{
  activation::{
    ActivationRecord, PlacementCommand, PlacementCommandResult, PlacementCoordinatorCore, PlacementCoordinatorError,
    PlacementEvent, PlacementLease, PlacementLocality, PlacementRequestId, RendezvousHasher,
    placement_lock_error::PlacementLockError,
  },
  grain::GrainKey,
};

#[test]
fn accepted_cache_hit_refreshes_idle_passivation_activity() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");
  coordinator.update_topology(vec!["node1:8080".to_string()]);
  let key = GrainKey::new("user/recent".to_string());
  let _ = coordinator.resolve(&key, 0).expect("activate");
  let _ = coordinator.drain_events();

  let _ = coordinator.resolve(&key, 9).expect("cache hit");
  coordinator.passivate_idle(10, 10);

  assert!(
    !coordinator
      .drain_events()
      .iter()
      .any(|event| matches!(event, PlacementEvent::Passivated { key: passivated, .. } if *passivated == key))
  );
}

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
    result:     Err(PlacementLockError::Failed { reason: "missing".to_string() }),
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
    .handle_command_result(PlacementCommandResult::LockReleased {
      request_id,
      result: Ok(()),
      completed_at_nanos: 1_000_000_000_000,
    })
    .expect("lock released");
  let resolution = outcome.resolution.expect("resolution");
  assert_eq!(resolution.pid, record.pid);
}

#[test]
fn distributed_activation_uses_completion_time_for_idle_passivation() {
  let mut coordinator = PlacementCoordinatorCore::new(128, 60);
  coordinator.start_member().expect("start");
  coordinator.set_distributed_activation(true);
  coordinator.update_topology(vec!["node1:8080".to_string()]);
  coordinator.set_local_authority("node1:8080".to_string());

  let key = GrainKey::new("user/slow".to_string());
  let outcome = coordinator.resolve_at(&key, 0, 0).expect("resolve");
  let PlacementCommand::TryAcquire { request_id, .. } = outcome.commands[0] else {
    panic!("expected TryAcquire");
  };
  let lease = PlacementLease { key: key.clone(), owner: "node1:8080".to_string(), expires_at: 10 };
  let _ = coordinator
    .handle_command_result(PlacementCommandResult::LockAcquired { request_id, result: Ok(lease) })
    .expect("lock acquired");
  let _ = coordinator
    .handle_command_result(PlacementCommandResult::ActivationLoaded { request_id, result: Ok(None) })
    .expect("activation loaded");
  let record = ActivationRecord::new("node1:8080::user/slow".to_string(), None, 0);
  let _ = coordinator
    .handle_command_result(PlacementCommandResult::ActivationEnsured { request_id, result: Ok(record) })
    .expect("activation ensured");
  let _ = coordinator
    .handle_command_result(PlacementCommandResult::ActivationStored { request_id, result: Ok(()) })
    .expect("activation stored");
  let _ = coordinator
    .handle_command_result(PlacementCommandResult::LockReleased {
      request_id,
      result: Ok(()),
      completed_at_nanos: 2_000_000_000,
    })
    .expect("lock released");
  let _ = coordinator.drain_events();

  coordinator.passivate_idle_at(2_500_000_000, 1_000_000_000);

  assert!(
    !coordinator
      .drain_events()
      .iter()
      .any(|event| matches!(event, PlacementEvent::Passivated { key: passivated, .. } if *passivated == key))
  );
}
