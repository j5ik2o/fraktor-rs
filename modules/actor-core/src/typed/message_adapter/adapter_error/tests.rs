use crate::typed::message_adapter::AdapterError;

#[test]
fn adapter_error_equality() {
  assert_eq!(AdapterError::RegistryFull, AdapterError::RegistryFull);
}
