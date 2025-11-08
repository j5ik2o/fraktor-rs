use alloc::string::String;

use crate::{NoStdToolbox, typed::message_adapter::AdapterPayload};

#[test]
fn payload_reports_original_type_id() {
  let payload = AdapterPayload::<NoStdToolbox>::new(42_u32);
  assert_eq!(payload.type_id(), core::any::TypeId::of::<u32>());
}

#[test]
fn payload_downcasts_to_requested_type() {
  let payload = AdapterPayload::<NoStdToolbox>::new(String::from("hello"));
  let concrete = payload.clone().try_downcast::<String>().expect("downcast succeeds");
  assert_eq!(&*concrete, "hello");

  let original = payload.try_downcast::<u32>().expect_err("downcast fails");
  assert_eq!(original.type_id(), core::any::TypeId::of::<String>());
}
