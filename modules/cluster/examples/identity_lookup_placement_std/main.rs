#![allow(clippy::print_stdout)]

//! Identity lookup placement demo (std driver).
//!
//! This example exercises the std driver with async lock/storage/executor
//! implementations and publishes placement events to the event stream.
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example identity_lookup_placement_std --features std
//! ```

#[cfg(not(feature = "std"))]
compile_error!("identity_lookup_placement_std は --features std が必要です。");

use std::{collections::HashMap, future::Future, pin::Pin};

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle,
};
use fraktor_cluster_rs::{
  core::{
    ActivationEntry, ActivationError, ActivationRecord, ActivationStorageError, GrainKey, LookupError,
    PlacementCoordinatorCore, PlacementCoordinatorSharedGeneric, PlacementEvent, PlacementLease, PlacementLockError,
    PlacementResolution, RendezvousHasher,
  },
  std::{ActivationExecutor, ActivationStorage, PlacementCoordinatorDriverGeneric, PlacementLock},
};
use fraktor_utils_rs::{core::sync::SharedAccess, std::runtime_toolbox::StdToolbox};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct PlacementEventLogger {
  label: &'static str,
}

impl PlacementEventLogger {
  const fn new(label: &'static str) -> Self {
    Self { label }
  }
}

impl EventStreamSubscriber<StdToolbox> for PlacementEventLogger {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(event) = payload.payload().downcast_ref::<PlacementEvent>() {
          println!("[{}][event] {event:?}", self.label);
        }
      }
    }
  }
}

struct DemoLock {
  fail_first: bool,
}

impl DemoLock {
  const fn new(fail_first: bool) -> Self {
    Self { fail_first }
  }
}

impl PlacementLock for DemoLock {
  fn try_acquire<'a>(
    &'a mut self,
    key: &'a GrainKey,
    owner: &'a str,
    now: u64,
  ) -> BoxFuture<'a, Result<PlacementLease, PlacementLockError>> {
    let fail = self.fail_first;
    self.fail_first = false;
    let key = key.clone();
    let owner = owner.to_string();
    Box::pin(async move {
      if fail {
        Err(PlacementLockError::Failed { reason: "demo lock denied".to_string() })
      } else {
        Ok(PlacementLease { key, owner, expires_at: now + 30 })
      }
    })
  }

  fn release<'a>(&'a mut self, _lease: PlacementLease) -> BoxFuture<'a, Result<(), PlacementLockError>> {
    Box::pin(async move { Ok(()) })
  }
}

#[derive(Default)]
struct DemoStorage {
  entries: HashMap<String, ActivationEntry>,
}

impl ActivationStorage for DemoStorage {
  fn load<'a>(
    &'a mut self,
    key: &'a GrainKey,
  ) -> BoxFuture<'a, Result<Option<ActivationEntry>, ActivationStorageError>> {
    let entry = self.entries.get(key.value()).cloned();
    Box::pin(async move { Ok(entry) })
  }

  fn store<'a>(
    &'a mut self,
    key: &'a GrainKey,
    entry: ActivationEntry,
  ) -> BoxFuture<'a, Result<(), ActivationStorageError>> {
    let key = key.value().to_string();
    Box::pin(async move {
      self.entries.insert(key, entry);
      Ok(())
    })
  }

  fn remove<'a>(&'a mut self, key: &'a GrainKey) -> BoxFuture<'a, Result<(), ActivationStorageError>> {
    let key = key.value().to_string();
    Box::pin(async move {
      self.entries.remove(&key);
      Ok(())
    })
  }
}

struct DemoExecutor {
  version: u64,
}

impl DemoExecutor {
  const fn new() -> Self {
    Self { version: 0 }
  }
}

impl ActivationExecutor for DemoExecutor {
  fn ensure_activation<'a>(
    &'a mut self,
    key: &'a GrainKey,
    owner: &'a str,
  ) -> BoxFuture<'a, Result<ActivationRecord, ActivationError>> {
    self.version = self.version.saturating_add(1);
    let version = self.version;
    let pid = format!("{owner}::{}", key.value());
    Box::pin(async move { Ok(ActivationRecord::new(pid, None, version)) })
  }
}

#[tokio::main]
async fn main() {
  println!("=== Identity Lookup Placement Driver Demo (std) ===");

  let authorities = vec!["node-a".to_string(), "node-b".to_string()];
  let local = "node-a";
  let now = 1_000;

  let key_local = select_key_for_authority(local, &authorities, 0);
  let key_remote = select_key_for_authority("node-b", &authorities, 200);

  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let _subscription = subscribe_placement_events(&event_stream, "driver");

  let mut coordinator = PlacementCoordinatorCore::new(16, 30);
  coordinator.start_member().expect("start_member");
  coordinator.set_local_authority(local.to_string());
  coordinator.update_topology(authorities.clone());
  coordinator.set_distributed_activation(true);
  let coordinator_shared = PlacementCoordinatorSharedGeneric::<StdToolbox>::new(coordinator);

  let snapshot = coordinator_shared.with_read(|core| core.snapshot());
  println!(
    "[snapshot] state={:?} authorities={:?} local={:?}",
    snapshot.state, snapshot.authorities, snapshot.local_authority
  );

  // ストレージの基本操作（store/remove）を事前に確認
  let mut storage = DemoStorage::default();
  let seed_key = GrainKey::new("seed:1".to_string());
  let seed_entry = ActivationEntry {
    owner:       local.to_string(),
    record:      ActivationRecord::new(format!("{local}::seed:1"), None, 0),
    observed_at: now,
  };
  storage.store(&seed_key, seed_entry).await.expect("storage store");
  storage.remove(&seed_key).await.expect("storage remove");
  println!("[storage] seed entry store/remove done");

  let lock = DemoLock::new(true);
  let executor = DemoExecutor::new();
  let mut driver =
    PlacementCoordinatorDriverGeneric::new(coordinator_shared.clone(), lock, storage, executor, event_stream.clone());

  println!("\n--- Resolve with lock denial ---");
  match driver.resolve(&key_local, now).await {
    | Ok(resolution) => println!("[unexpected] resolved: {resolution:?}"),
    | Err(LookupError::Pending) => println!("[expected] lock denied -> pending"),
    | Err(err) => println!("[unexpected] error: {err:?}"),
  }

  println!("\n--- Resolve after lock success ---");
  let resolution = driver.resolve(&key_local, now + 1).await.expect("resolve after lock");
  print_resolution("local", &resolution);

  // キャッシュ削除後の再解決でストレージロードを通す
  coordinator_shared.with_write(|core| core.remove_pid(&key_local));
  println!("\n--- Resolve with storage hit ---");
  let resolution = driver.resolve(&key_local, now + 2).await.expect("resolve with storage");
  print_resolution("storage-hit", &resolution);

  println!("\n--- Resolve remote key ---");
  let resolution = driver.resolve(&key_remote, now + 3).await.expect("resolve remote");
  print_resolution("remote", &resolution);

  println!("\n=== Demo complete ===");
}

fn subscribe_placement_events(
  event_stream: &EventStreamSharedGeneric<StdToolbox>,
  label: &'static str,
) -> fraktor_actor_rs::core::event::stream::EventStreamSubscriptionGeneric<StdToolbox> {
  let subscriber = subscriber_handle(PlacementEventLogger::new(label));
  event_stream.subscribe(&subscriber)
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

fn print_resolution(label: &str, resolution: &PlacementResolution) {
  println!(
    "[{label}] pid={} locality={:?} owner={}",
    resolution.pid, resolution.locality, resolution.decision.authority
  );
}
