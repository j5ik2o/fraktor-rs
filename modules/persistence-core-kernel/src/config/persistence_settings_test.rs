use crate::{config::PersistenceSettings, journal::JournalActorConfig, snapshot::SnapshotActorConfig};

#[test]
fn default_settings_match_existing_runtime_actor_defaults() {
  let settings = PersistenceSettings::default();

  assert_eq!(settings.journal_actor_config(), JournalActorConfig::default());
  assert_eq!(settings.snapshot_actor_config(), SnapshotActorConfig::default());
}

#[test]
fn settings_builder_updates_runtime_actor_configs() {
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(3))
    .with_snapshot_actor_config(SnapshotActorConfig::new(5));

  assert_eq!(settings.journal_actor_config().retry_max(), 3);
  assert_eq!(settings.snapshot_actor_config().retry_max(), 5);
}
