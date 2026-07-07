use crate::activation::{InMemoryRememberEntitiesStore, RememberEntitiesStore};

#[test]
fn trait_object_can_be_used_through_port() {
  let mut store: Box<dyn RememberEntitiesStore> = Box::new(InMemoryRememberEntitiesStore::new());
  store.add_entity(String::from("entity-1")).expect("add");
  assert!(store.list_entities().expect("list").contains("entity-1"));
}
