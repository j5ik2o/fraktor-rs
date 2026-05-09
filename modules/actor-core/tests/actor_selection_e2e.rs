#![cfg(not(target_os = "none"))]

mod common;

use core::time::Duration;
use std::vec::Vec;

use common::wait_until;
use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext, ChildRef,
    actor_path::ActorPath,
    actor_ref_provider::{ActorRefResolveError, LocalActorRefProviderInstaller},
    actor_selection::ActorSelectionError,
    error::ActorError,
    messaging::{ActorIdentity, AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, SpinBlocker},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SpinSyncMutex};

struct SpawnWorker;
struct Deliver(u32);

struct NoopGuardian;

impl Actor for NoopGuardian {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct Worker {
  deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl Worker {
  fn new(deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { deliveries }
  }
}

impl Actor for Worker {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(deliver) = message.downcast_ref::<Deliver>() {
      self.deliveries.lock().push(deliver.0);
    }
    Ok(())
  }
}

struct Parent {
  deliveries:  ArcShared<SpinSyncMutex<Vec<u32>>>,
  worker_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  path_slot:   ArcShared<SpinSyncMutex<Option<ActorPath>>>,
}

impl Parent {
  fn new(
    deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>,
    worker_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    path_slot: ArcShared<SpinSyncMutex<Option<ActorPath>>>,
  ) -> Self {
    Self { deliveries, worker_slot, path_slot }
  }
}

impl Actor for Parent {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnWorker>().is_some() && self.worker_slot.lock().is_none() {
      let deliveries = self.deliveries.clone();
      let worker = ctx
        .spawn_child(&Props::from_fn(move || Worker::new(deliveries.clone())).with_name("worker"))
        .map_err(|error| ActorError::recoverable(format!("spawn worker failed: {error:?}")))?;
      let path = worker.actor_ref().path().ok_or_else(|| ActorError::recoverable("worker path missing"))?;
      self.path_slot.lock().replace(path);
      self.worker_slot.lock().replace(worker);
    }
    Ok(())
  }
}

const SELECTION_TIMEOUT_MS: u64 = 2_000;

#[test]
fn actor_selection_resolves_string_and_direct_path_targets() {
  let deliveries = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let worker_slot = ArcShared::new(SpinSyncMutex::new(None));
  let path_slot = ArcShared::new(SpinSyncMutex::new(None));
  let system = ActorSystem::create_from_props(
    &Props::from_fn(|| NoopGuardian),
    ActorSystemConfig::new(TestTickDriver::default())
      .with_system_name("selection-e2e")
      .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default()),
  )
  .expect("system");

  let mut parent = system
    .actor_of_named(
      &Props::from_fn({
        let deliveries = deliveries.clone();
        let worker_slot = worker_slot.clone();
        let path_slot = path_slot.clone();
        move || Parent::new(deliveries.clone(), worker_slot.clone(), path_slot.clone())
      }),
      "selection-parent",
    )
    .expect("parent");
  parent.tell(AnyMessage::new(SpawnWorker));

  assert!(wait_until(SELECTION_TIMEOUT_MS, || { path_slot.lock().is_some() && worker_slot.lock().is_some() }));
  let worker = worker_slot.lock().clone().expect("worker");
  let worker_pid = worker.pid();
  let worker_path = path_slot.lock().clone().expect("worker path");
  let selection_path = worker_path.to_relative_string();

  system.actor_selection(&selection_path).tell(AnyMessage::new(Deliver(1)), None).expect("string path selection tell");
  system.actor_selection_from_path(&worker_path).tell(AnyMessage::new(Deliver(2)), None).expect("direct path tell");

  assert!(wait_until(SELECTION_TIMEOUT_MS, || *deliveries.lock() == vec![1, 2]));

  let response = system
    .actor_selection_from_path(&worker_path)
    .resolve_one(Duration::from_millis(SELECTION_TIMEOUT_MS))
    .expect("resolve one");
  assert!(wait_until(SELECTION_TIMEOUT_MS, || response.future().with_read(|future| future.is_ready())));
  let result =
    response.future().with_write(|future| future.try_take()).expect("identity future").expect("identity response");
  let identity = result.downcast_ref::<ActorIdentity>().expect("ActorIdentity");
  assert_eq!(identity.actor_ref().expect("resolved actor").pid(), worker_pid);

  let missing_path =
    selection_path.rsplit_once('/').map(|(parent, _)| format!("{parent}/missing")).expect("parent path");
  let missing = system.actor_selection(&missing_path);
  let error = missing.tell(AnyMessage::new(Deliver(99)), None).expect_err("missing path should fail");
  assert!(matches!(error, ActorSelectionError::Resolve(ActorRefResolveError::NotFound(_))));

  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}
