//! std-only driver for placement coordination.

use alloc::string::String;

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamSharedGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::{
  core::{
    GrainKey, LookupError, PlacementCommand, PlacementCommandResult, PlacementCoordinatorSharedGeneric,
    PlacementResolution,
  },
  std::{
    activation_executor::ActivationExecutor, activation_storage::ActivationStorage, placement_lock::PlacementLock,
  },
};

/// Driver that executes placement commands via std implementations.
pub struct PlacementCoordinatorDriverGeneric<
  TB: RuntimeToolbox + 'static,
  TLock: PlacementLock,
  TStorage: ActivationStorage,
  TExecutor: ActivationExecutor,
> {
  coordinator:  PlacementCoordinatorSharedGeneric<TB>,
  lock:         TLock,
  storage:      TStorage,
  executor:     TExecutor,
  event_stream: EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static, TLock: PlacementLock, TStorage: ActivationStorage, TExecutor: ActivationExecutor>
  PlacementCoordinatorDriverGeneric<TB, TLock, TStorage, TExecutor>
{
  /// Creates a new driver.
  #[must_use]
  pub fn new(
    coordinator: PlacementCoordinatorSharedGeneric<TB>,
    lock: TLock,
    storage: TStorage,
    executor: TExecutor,
    event_stream: EventStreamSharedGeneric<TB>,
  ) -> Self {
    Self { coordinator, lock, storage, executor, event_stream }
  }

  /// Resolves placement and executes required commands.
  ///
  /// # Errors
  ///
  /// Returns an error when placement cannot be resolved.
  pub async fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let mut outcome = self.coordinator.with_write(|coordinator| coordinator.resolve(key, now))?;
    self.publish_events();

    loop {
      if let Some(resolution) = outcome.resolution {
        return Ok(resolution);
      }
      if outcome.commands.is_empty() {
        return Err(LookupError::Pending);
      }

      let commands = core::mem::take(&mut outcome.commands);
      for command in commands {
        let result = self.execute_command(command).await;
        outcome = self
          .coordinator
          .with_write(|coordinator| coordinator.handle_command_result(result))
          .map_err(|_| LookupError::Pending)?;
        self.publish_events();
        if let Some(resolution) = outcome.resolution.clone() {
          return Ok(resolution);
        }
      }
    }
  }

  async fn execute_command(&mut self, command: PlacementCommand) -> PlacementCommandResult {
    match command {
      | PlacementCommand::TryAcquire { request_id, key, owner, now } => {
        let result = self.lock.try_acquire(&key, &owner, now).await;
        PlacementCommandResult::LockAcquired { request_id, result }
      },
      | PlacementCommand::LoadActivation { request_id, key } => {
        let result = self.storage.load(&key).await;
        PlacementCommandResult::ActivationLoaded { request_id, result }
      },
      | PlacementCommand::EnsureActivation { request_id, key, owner } => {
        let result = self.executor.ensure_activation(&key, &owner).await;
        PlacementCommandResult::ActivationEnsured { request_id, result }
      },
      | PlacementCommand::StoreActivation { request_id, key, entry } => {
        let result = self.storage.store(&key, entry).await;
        PlacementCommandResult::ActivationStored { request_id, result }
      },
      | PlacementCommand::Release { request_id, lease } => {
        let result = self.lock.release(lease).await;
        PlacementCommandResult::LockReleased { request_id, result }
      },
    }
  }

  fn publish_events(&self) {
    let events = self.coordinator.with_write(|coordinator| coordinator.drain_events());
    for event in events {
      let payload = AnyMessageGeneric::new(event);
      let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
      self.event_stream.publish(&extension_event);
    }
  }
}
