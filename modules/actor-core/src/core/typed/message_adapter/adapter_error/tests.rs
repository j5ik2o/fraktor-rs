use alloc::string::String;
use core::any::TypeId;

use crate::core::typed::message_adapter::AdapterError;

#[test]
fn adapter_error_equality() {
  assert_eq!(AdapterError::RegistryFull, AdapterError::RegistryFull);
  assert_eq!(AdapterError::TypeMismatch(TypeId::of::<u32>()), AdapterError::TypeMismatch(TypeId::of::<u32>()));
}

#[test]
fn adapter_error_debug() {
  let error = AdapterError::Custom(String::from("oops"));
  let formatted = alloc::format!("{:?}", error);
  assert!(formatted.contains("Custom"));
}
