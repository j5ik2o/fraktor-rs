//! Identity lookup and placement demo for no_std core.
#![allow(clippy::print_stdout)]

//! Identity lookup placement demo (no_std core).
//!
//! This example exercises the placement core without std-only drivers.
//! It demonstrates:
//! - PartitionIdentityLookup usage (resolve/topology/events)
//! - PlacementCoordinatorCore distributed activation flow
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example identity_lookup_placement_no_std --features test-support
//! ```

#[cfg(not(feature = "test-support"))]
compile_error!("identity_lookup_placement_no_std は --features test-support が必要です。");

use fraktor_cluster_rs::core::{
  ActivatedKind, ActivationRecord, GrainKey, IdentityLookup, LookupError, PartitionIdentityLookup,
  PartitionIdentityLookupConfig, PidCacheEvent, PlacementCommand, PlacementCommandResult, PlacementCoordinatorCore,
  PlacementCoordinatorOutcome, PlacementEvent, PlacementLease, PlacementLockError, PlacementResolution,
  RendezvousHasher,
};

fn main() {
  println!("=== Identity Lookup Placement Core Demo (no_std) ===");

  let authorities = vec!["node-a".to_string(), "node-b".to_string()];
  let local = "node-a";
  let now = 1_000;

  let key_local = select_key_for_authority(local, &authorities, 0);
  let key_remote = select_key_for_authority("node-b", &authorities, 100);

  println!("\n--- PartitionIdentityLookup ---");
  let mut lookup = PartitionIdentityLookup::new(PartitionIdentityLookupConfig::new(8, 5, 1));
  lookup.set_local_authority(local.to_string());
  lookup.update_topology(authorities.clone());
  lookup.setup_member(&[ActivatedKind::new("grain")]).expect("setup_member");

  print_resolution("local", lookup.resolve(&key_local, now));
  print_resolution("remote", lookup.resolve(&key_remote, now));

  print_events("events", lookup.drain_events());
  print_cache_events("cache events", lookup.drain_cache_events());

  // トポロジ変更とパッシベートの動作確認
  lookup.on_member_left("node-b");
  lookup.passivate_idle(now + 10, lookup.config().idle_ttl_secs());
  lookup.remove_pid(&key_local);
  print_events("after passivation", lookup.drain_events());
  print_cache_events("cache events after passivation", lookup.drain_cache_events());

  println!("\n--- PlacementCoordinatorCore (distributed activation) ---");
  let mut coordinator = PlacementCoordinatorCore::new(16, 5);
  coordinator.start_member().expect("start_member");
  coordinator.set_local_authority(local.to_string());
  coordinator.update_topology(authorities.clone());
  coordinator.set_distributed_activation(true);

  let key_success = select_key_for_authority(local, &authorities, 200);
  let outcome = coordinator.resolve(&key_success, now).expect("resolve");
  let resolution = drive_outcome(&mut coordinator, outcome, false);
  println!("[resolved] {resolution:?}");
  print_events("distributed events", coordinator.drain_events());

  let key_denied = select_key_for_authority(local, &authorities, 300);
  let outcome = coordinator.resolve(&key_denied, now + 1).expect("resolve");
  let resolution = drive_outcome(&mut coordinator, outcome, true);
  println!("[denied] {resolution:?}");
  print_events("lock denied events", coordinator.drain_events());

  println!("\n=== Demo complete ===");
}

fn select_key_for_authority(authority: &str, authorities: &[String], seed: u64) -> GrainKey {
  for offset in 0..200u64 {
    let candidate = format!("grain:{seed}:{offset}");
    let key = GrainKey::new(candidate);
    if let Some(owner) = RendezvousHasher::select(authorities, &key) {
      if owner == authority {
        return key;
      }
    }
  }
  GrainKey::new(format!("fallback:{authority}"))
}

fn print_resolution(label: &str, result: Result<PlacementResolution, LookupError>) {
  match result {
    | Ok(resolution) => {
      println!(
        "[{label}] pid={} locality={:?} owner={}",
        resolution.pid, resolution.locality, resolution.decision.authority
      );
    },
    | Err(err) => println!("[{label}] resolve error: {err:?}"),
  }
}

fn print_events(label: &str, events: Vec<PlacementEvent>) {
  if events.is_empty() {
    println!("[{label}] (no events)");
    return;
  }
  for event in events {
    println!("[{label}] {event:?}");
  }
}

fn print_cache_events(label: &str, events: Vec<PidCacheEvent>) {
  if events.is_empty() {
    println!("[{label}] (no cache events)");
    return;
  }
  for event in events {
    println!("[{label}] {event:?}");
  }
}

fn drive_outcome(
  coordinator: &mut PlacementCoordinatorCore,
  mut outcome: PlacementCoordinatorOutcome,
  deny_lock: bool,
) -> Option<PlacementResolution> {
  loop {
    if let Some(resolution) = outcome.resolution {
      return Some(resolution);
    }
    if outcome.commands.is_empty() {
      return None;
    }

    let commands = core::mem::take(&mut outcome.commands);
    for command in commands {
      println!("[driver] command={}", command_name(&command));
      let result = execute_command(command, deny_lock);
      outcome = coordinator.handle_command_result(result).expect("handle_command_result");
    }
  }
}

fn command_name(command: &PlacementCommand) -> &'static str {
  match command {
    | PlacementCommand::TryAcquire { .. } => "try_acquire",
    | PlacementCommand::LoadActivation { .. } => "load_activation",
    | PlacementCommand::EnsureActivation { .. } => "ensure_activation",
    | PlacementCommand::StoreActivation { .. } => "store_activation",
    | PlacementCommand::Release { .. } => "release",
  }
}

fn execute_command(command: PlacementCommand, deny_lock: bool) -> PlacementCommandResult {
  match command {
    | PlacementCommand::TryAcquire { request_id, key, owner, now } => {
      if deny_lock {
        PlacementCommandResult::LockAcquired {
          request_id,
          result: Err(PlacementLockError::Failed { reason: "demo lock denied".to_string() }),
        }
      } else {
        let lease = PlacementLease { key, owner, expires_at: now + 30 };
        PlacementCommandResult::LockAcquired { request_id, result: Ok(lease) }
      }
    },
    | PlacementCommand::LoadActivation { request_id, .. } => {
      PlacementCommandResult::ActivationLoaded { request_id, result: Ok(None) }
    },
    | PlacementCommand::EnsureActivation { request_id, key, owner } => {
      let pid = format!("{owner}::{}", key.value());
      let record = ActivationRecord::new(pid, None, 1);
      PlacementCommandResult::ActivationEnsured { request_id, result: Ok(record) }
    },
    | PlacementCommand::StoreActivation { request_id, entry, .. } => {
      println!("[driver] store pid={}", entry.record.pid);
      PlacementCommandResult::ActivationStored { request_id, result: Ok(()) }
    },
    | PlacementCommand::Release { request_id, .. } => {
      PlacementCommandResult::LockReleased { request_id, result: Ok(()) }
    },
  }
}
