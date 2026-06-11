//! Public persistence runtime configuration.

#[cfg(test)]
#[path = "persistence_config_test.rs"]
mod tests;

use crate::{journal::JournalActorConfig, snapshot::SnapshotActorConfig};

/// Public configuration for the persistence runtime actors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistenceConfig {
  journal_actor_config:  JournalActorConfig,
  snapshot_actor_config: SnapshotActorConfig,
}

impl PersistenceConfig {
  /// Creates a configuration from the default store actor configs.
  #[must_use]
  pub const fn default_config() -> Self {
    Self::new(JournalActorConfig::default_config(), SnapshotActorConfig::default_config())
  }

  /// Creates a configuration with explicit store actor configs.
  #[must_use]
  pub const fn new(journal_actor_config: JournalActorConfig, snapshot_actor_config: SnapshotActorConfig) -> Self {
    Self { journal_actor_config, snapshot_actor_config }
  }

  /// Returns the journal actor configuration.
  #[must_use]
  pub const fn journal_actor_config(&self) -> JournalActorConfig {
    self.journal_actor_config
  }

  /// Returns the snapshot actor configuration.
  #[must_use]
  pub const fn snapshot_actor_config(&self) -> SnapshotActorConfig {
    self.snapshot_actor_config
  }

  /// Updates the journal actor configuration.
  #[must_use]
  pub const fn with_journal_actor_config(mut self, journal_actor_config: JournalActorConfig) -> Self {
    self.journal_actor_config = journal_actor_config;
    self
  }

  /// Updates the snapshot actor configuration.
  #[must_use]
  pub const fn with_snapshot_actor_config(mut self, snapshot_actor_config: SnapshotActorConfig) -> Self {
    self.snapshot_actor_config = snapshot_actor_config;
    self
  }
}

impl Default for PersistenceConfig {
  fn default() -> Self {
    Self::default_config()
  }
}
