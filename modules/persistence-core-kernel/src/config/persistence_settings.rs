//! Public persistence runtime settings.

#[cfg(test)]
#[path = "persistence_settings_test.rs"]
mod tests;

use crate::{journal::JournalActorConfig, snapshot::SnapshotActorConfig};

/// Public settings for the persistence runtime actors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistenceSettings {
  journal_actor_config:  JournalActorConfig,
  snapshot_actor_config: SnapshotActorConfig,
}

impl PersistenceSettings {
  /// Creates settings from the default store actor configs.
  #[must_use]
  pub const fn default_settings() -> Self {
    Self::new(JournalActorConfig::default_config(), SnapshotActorConfig::default_config())
  }

  /// Creates settings with explicit store actor configs.
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

impl Default for PersistenceSettings {
  fn default() -> Self {
    Self::default_settings()
  }
}
