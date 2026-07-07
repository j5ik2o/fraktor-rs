use alloc::string::String;

use super::RememberEntitiesStoreError;

#[test]
fn display_messages_are_stable() {
  assert_eq!(
    RememberEntitiesStoreError::InvalidEntityId { entity_id: String::from("bad") }.to_string(),
    "invalid entity id: bad"
  );
  assert_eq!(
    RememberEntitiesStoreError::NotFound { entity_id: String::from("missing") }.to_string(),
    "remembered entity not found: missing"
  );
}
