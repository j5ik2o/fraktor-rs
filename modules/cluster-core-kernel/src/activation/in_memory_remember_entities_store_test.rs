use alloc::string::String;

use super::{InMemoryRememberEntitiesStore, RememberEntitiesStore, RememberEntitiesStoreError};

#[test]
fn add_list_and_remove_entities() {
  let mut store = InMemoryRememberEntitiesStore::new();
  store.add_entity(String::from("entity-1")).expect("add");
  store.add_entity(String::from("entity-2")).expect("add");

  let entities = store.list_entities().expect("list");
  assert_eq!(entities.len(), 2);
  assert!(entities.contains("entity-1"));

  store.remove_entity(String::from("entity-1")).expect("remove");
  let entities = store.list_entities().expect("list");
  assert_eq!(entities, alloc::collections::BTreeSet::from([String::from("entity-2")]));
}

#[test]
fn rejects_empty_entity_ids() {
  let mut store = InMemoryRememberEntitiesStore::new();
  let error = store.add_entity(String::new()).expect_err("empty id");
  assert_eq!(error, RememberEntitiesStoreError::InvalidEntityId { entity_id: String::new() });
}

#[test]
fn remove_missing_entity_reports_not_found() {
  let mut store = InMemoryRememberEntitiesStore::new();
  let error = store.remove_entity(String::from("missing")).expect_err("missing");
  assert_eq!(error, RememberEntitiesStoreError::NotFound { entity_id: String::from("missing") });
}
