use super::RememberEntitiesStoreMode;

#[test]
fn default_mode_is_ddata() {
  assert_eq!(RememberEntitiesStoreMode::default(), RememberEntitiesStoreMode::DData);
}

#[test]
fn parses_known_configuration_strings() {
  assert_eq!(RememberEntitiesStoreMode::parse("ddata"), Some(RememberEntitiesStoreMode::DData));
  assert_eq!(RememberEntitiesStoreMode::parse("eventsourced"), Some(RememberEntitiesStoreMode::EventSourced));
  assert_eq!(RememberEntitiesStoreMode::parse("custom"), Some(RememberEntitiesStoreMode::Custom));
  assert_eq!(RememberEntitiesStoreMode::parse("unknown"), None);
}

#[test]
fn serializes_to_configuration_strings() {
  assert_eq!(RememberEntitiesStoreMode::DData.as_str(), "ddata");
  assert_eq!(RememberEntitiesStoreMode::EventSourced.as_str(), "eventsourced");
  assert_eq!(RememberEntitiesStoreMode::Custom.as_str(), "custom");
}
