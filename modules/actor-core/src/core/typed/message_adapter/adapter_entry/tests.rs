use fraktor_utils_core_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::typed::message_adapter::{AdapterEntry, AdapterOutcome};

#[test]
fn adapter_entry_type_id_round_trip() {
  let entry =
    AdapterEntry::<i32, NoStdToolbox>::new::<u32, _>(core::any::TypeId::of::<u32>(), |value| Ok(value as i32));
  assert_eq!(entry.type_id(), core::any::TypeId::of::<u32>());
}

#[test]
fn adapter_entry_executes_handler() {
  let entry = AdapterEntry::<i32, NoStdToolbox>::new::<alloc::string::String, _>(
    core::any::TypeId::of::<alloc::string::String>(),
    |value| {
      value.parse::<i32>().map_err(|_| crate::core::typed::message_adapter::AdapterFailure::Custom("parse".into()))
    },
  );
  let payload =
    crate::core::typed::message_adapter::AdapterPayload::<NoStdToolbox>::new(alloc::string::String::from("12"));
  assert_eq!(entry.invoke(payload), AdapterOutcome::Converted(12));
}
