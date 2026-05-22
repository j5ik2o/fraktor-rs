use super::SerializationRegistryContributionError;
use crate::serialization::{SerializationError, SerializerId};

#[test]
fn contribution_error_exposes_message() {
  let error =
    SerializationRegistryContributionError::new(SerializationError::UnknownSerializer(SerializerId::from_raw(100)));

  assert!(matches!(error.message(), SerializationError::UnknownSerializer(id) if *id == SerializerId::from_raw(100)));
}
