//! Actor-system scoped in-memory persistence store.

use alloc::{
  collections::BTreeMap,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::any::Any;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{Extension, ExtensionId},
  system::ActorSystem,
};
use fraktor_actor_core_typed_rs::TypedActorSystem;
use fraktor_persistence_core_kernel_rs::error::PersistenceError;
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::PersistenceEffectorConfig;

struct EphemeralPersistenceStoreId;

struct EphemeralPersistedEvent {
  sequence_nr: u64,
  payload:     ArcShared<dyn Any + Send + Sync>,
}

struct EphemeralPersistedSnapshot {
  sequence_nr: u64,
  payload:     ArcShared<dyn Any + Send + Sync>,
}

#[derive(Default)]
struct EphemeralPersistenceEntry {
  sequence_nr: u64,
  events:      Vec<EphemeralPersistedEvent>,
  snapshots:   Vec<EphemeralPersistedSnapshot>,
}

struct EphemeralRecovery {
  sequence_nr: u64,
  snapshot:    Option<ArcShared<dyn Any + Send + Sync>>,
  events:      Vec<ArcShared<dyn Any + Send + Sync>>,
}

/// Stores events and snapshots within one actor system.
pub(crate) struct EphemeralPersistenceStore {
  entries: SharedLock<BTreeMap<String, EphemeralPersistenceEntry>>,
}

impl EphemeralPersistenceStoreId {
  const fn new() -> Self {
    Self
  }
}

impl EphemeralPersistenceStore {
  pub(crate) fn for_system<M>(system: &TypedActorSystem<M>) -> ArcShared<Self>
  where
    M: Send + Sync + 'static, {
    system.register_extension(&EphemeralPersistenceStoreId::new())
  }

  pub(crate) fn recover<S, E, M>(
    &self,
    config: &PersistenceEffectorConfig<S, E, M>,
  ) -> Result<(S, u64), PersistenceError>
  where
    S: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
    M: Send + Sync + 'static, {
    let recovery = self.recovery_payloads(config.persistence_id().as_str());
    let mut state = match recovery.snapshot {
      | Some(snapshot) => snapshot
        .downcast_ref::<S>()
        .cloned()
        .ok_or_else(|| Self::recovery_error("snapshot payload type mismatch", config.persistence_id().as_str()))?,
      | None => config.initial_state().clone(),
    };

    for payload in recovery.events.iter() {
      let event = payload
        .downcast_ref::<E>()
        .ok_or_else(|| Self::recovery_error("event payload type mismatch", config.persistence_id().as_str()))?;
      state = config.apply_event(&state, event);
    }

    Ok((state, recovery.sequence_nr))
  }

  pub(crate) fn persist_events<S, E, M>(
    &self,
    config: &PersistenceEffectorConfig<S, E, M>,
    events: Vec<E>,
  ) -> Result<(Vec<E>, u64), PersistenceError>
  where
    E: Clone + Send + Sync + 'static, {
    let sequence_nr = self.entries.with_lock(|entries| {
      let entry = entries.entry(config.persistence_id().as_str().to_string()).or_default();
      for event in events.iter() {
        entry.sequence_nr = entry.sequence_nr.saturating_add(1);
        let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(event.clone());
        entry.events.push(EphemeralPersistedEvent { sequence_nr: entry.sequence_nr, payload });
      }
      entry.sequence_nr
    });
    Ok((events, sequence_nr))
  }

  pub(crate) fn persist_snapshot<S, E, M>(
    &self,
    config: &PersistenceEffectorConfig<S, E, M>,
    snapshot: S,
    sequence_nr: u64,
  ) -> Result<S, PersistenceError>
  where
    S: Clone + Send + Sync + 'static, {
    self.entries.with_lock(|entries| {
      let entry = entries.entry(config.persistence_id().as_str().to_string()).or_default();
      let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(snapshot.clone());
      entry.snapshots.push(EphemeralPersistedSnapshot { sequence_nr, payload });
    });
    Ok(snapshot)
  }

  pub(crate) fn delete_snapshots_to(&self, persistence_id: &str, to_sequence_nr: u64) -> Result<(), PersistenceError> {
    self.entries.with_lock(|entries| {
      if let Some(entry) = entries.get_mut(persistence_id) {
        entry.snapshots.retain(|snapshot| snapshot.sequence_nr > to_sequence_nr);
      }
    });
    Ok(())
  }

  fn new() -> Self {
    Self { entries: SharedLock::new_with_driver::<DefaultMutex<_>>(BTreeMap::new()) }
  }

  fn recovery_payloads(&self, persistence_id: &str) -> EphemeralRecovery {
    self.entries.with_lock(|entries| {
      let Some(entry) = entries.get(persistence_id) else {
        return EphemeralRecovery { sequence_nr: 0, snapshot: None, events: Vec::new() };
      };

      let snapshot = entry.snapshots.iter().max_by_key(|snapshot| snapshot.sequence_nr);
      let snapshot_seq = snapshot.map(|snapshot| snapshot.sequence_nr).unwrap_or(0);
      let events = entry
        .events
        .iter()
        .filter(|event| event.sequence_nr > snapshot_seq)
        .map(|event| event.payload.clone())
        .collect();

      EphemeralRecovery {
        sequence_nr: entry.sequence_nr,
        snapshot: snapshot.map(|snapshot| snapshot.payload.clone()),
        events,
      }
    })
  }

  fn recovery_error(reason: &str, persistence_id: &str) -> PersistenceError {
    PersistenceError::Recovery(format!("{reason}: {persistence_id}"))
  }
}

impl Extension for EphemeralPersistenceStore {}

impl ExtensionId for EphemeralPersistenceStoreId {
  type Ext = EphemeralPersistenceStore;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    EphemeralPersistenceStore::new()
  }
}
