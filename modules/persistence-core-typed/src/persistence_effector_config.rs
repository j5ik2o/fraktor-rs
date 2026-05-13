//! Persistence effector configuration.

use alloc::string::String;

use fraktor_persistence_core_kernel_rs::PersistenceError;
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  BackoffConfig, PersistenceEffectorMessageAdapter, PersistenceId, PersistenceMode, RetentionCriteria, SnapshotCriteria,
};

type ApplyEvent<S, E> = dyn Fn(&S, &E) -> S + Send + Sync;

/// Configuration used to build a typed persistence effector.
pub struct PersistenceEffectorConfig<S, E, M> {
  persistence_id:     PersistenceId,
  initial_state:      S,
  apply_event:        ArcShared<ApplyEvent<S, E>>,
  persistence_mode:   PersistenceMode,
  stash_capacity:     usize,
  snapshot_criteria:  SnapshotCriteria<S, E>,
  retention_criteria: RetentionCriteria,
  backoff_config:     BackoffConfig,
  message_adapter:    Option<PersistenceEffectorMessageAdapter<S, E, M>>,
}

impl<S, E, M> PersistenceEffectorConfig<S, E, M> {
  /// Creates a configuration with persisted mode and no automatic snapshots.
  #[must_use]
  pub fn new<F>(persistence_id: PersistenceId, initial_state: S, apply_event: F) -> Self
  where
    F: Fn(&S, &E) -> S + Send + Sync + 'static, {
    Self {
      persistence_id,
      initial_state,
      apply_event: ArcShared::new(apply_event),
      persistence_mode: PersistenceMode::Persisted,
      stash_capacity: usize::MAX,
      snapshot_criteria: SnapshotCriteria::never(),
      retention_criteria: RetentionCriteria::none(),
      backoff_config: BackoffConfig::default(),
      message_adapter: None,
    }
  }

  /// Returns the persistence id.
  #[must_use]
  pub const fn persistence_id(&self) -> &PersistenceId {
    &self.persistence_id
  }

  /// Returns the initial state.
  #[must_use]
  pub const fn initial_state(&self) -> &S {
    &self.initial_state
  }

  /// Applies one event to a state.
  #[must_use]
  pub fn apply_event(&self, state: &S, event: &E) -> S {
    (self.apply_event)(state, event)
  }

  /// Returns the selected persistence mode.
  #[must_use]
  pub const fn persistence_mode(&self) -> PersistenceMode {
    self.persistence_mode
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    self.stash_capacity
  }

  /// Returns the snapshot criteria.
  #[must_use]
  pub const fn snapshot_criteria(&self) -> &SnapshotCriteria<S, E> {
    &self.snapshot_criteria
  }

  /// Returns the retention criteria.
  #[must_use]
  pub const fn retention_criteria(&self) -> &RetentionCriteria {
    &self.retention_criteria
  }

  /// Returns the backoff configuration.
  #[must_use]
  pub const fn backoff_config(&self) -> &BackoffConfig {
    &self.backoff_config
  }

  /// Returns the optional message adapter.
  #[must_use]
  pub const fn message_adapter(&self) -> Option<&PersistenceEffectorMessageAdapter<S, E, M>> {
    self.message_adapter.as_ref()
  }

  /// Returns a config with the selected persistence mode.
  #[must_use]
  pub const fn with_persistence_mode(mut self, persistence_mode: PersistenceMode) -> Self {
    self.persistence_mode = persistence_mode;
    self
  }

  /// Returns a config with the selected stash capacity.
  #[must_use]
  pub const fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.stash_capacity = stash_capacity;
    self
  }

  /// Returns a config with the selected snapshot criteria.
  #[must_use]
  pub fn with_snapshot_criteria(mut self, snapshot_criteria: SnapshotCriteria<S, E>) -> Self {
    self.snapshot_criteria = snapshot_criteria;
    self
  }

  /// Returns a config with the selected retention criteria.
  #[must_use]
  pub const fn with_retention_criteria(mut self, retention_criteria: RetentionCriteria) -> Self {
    self.retention_criteria = retention_criteria;
    self
  }

  /// Returns a config with the selected backoff configuration.
  #[must_use]
  pub const fn with_backoff_config(mut self, backoff_config: BackoffConfig) -> Self {
    self.backoff_config = backoff_config;
    self
  }

  /// Returns a config with the selected message adapter.
  #[must_use]
  pub fn with_message_adapter(mut self, message_adapter: PersistenceEffectorMessageAdapter<S, E, M>) -> Self {
    self.message_adapter = Some(message_adapter);
    self
  }

  /// Validates the configuration.
  pub fn validate(&self) -> Result<(), PersistenceError> {
    if self.stash_capacity == 0 {
      return Err(validation_error("stash_capacity must be greater than 0"));
    }
    if let SnapshotCriteria::Every { number_of_events } = &self.snapshot_criteria
      && *number_of_events == 0
    {
      return Err(validation_error("snapshot interval must be greater than 0"));
    }
    if let Some(number_of_events) = self.retention_criteria.snapshot_every_interval()
      && number_of_events == 0
    {
      return Err(validation_error("retention snapshot interval must be greater than 0"));
    }
    if let Some(keep_snapshots) = self.retention_criteria.keep_snapshots()
      && keep_snapshots == 0
    {
      return Err(validation_error("retention keep_snapshots must be greater than 0"));
    }
    Ok(())
  }
}

impl<S, E, M> Clone for PersistenceEffectorConfig<S, E, M>
where
  S: Clone,
{
  fn clone(&self) -> Self {
    Self {
      persistence_id:     self.persistence_id.clone(),
      initial_state:      self.initial_state.clone(),
      apply_event:        self.apply_event.clone(),
      persistence_mode:   self.persistence_mode,
      stash_capacity:     self.stash_capacity,
      snapshot_criteria:  self.snapshot_criteria.clone(),
      retention_criteria: self.retention_criteria,
      backoff_config:     self.backoff_config.clone(),
      message_adapter:    self.message_adapter.clone(),
    }
  }
}

fn validation_error(message: &str) -> PersistenceError {
  PersistenceError::StateMachine(String::from(message))
}
