use fraktor_utils_core_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::typed::message_adapter::{AdaptMessage, AdapterOutcome};

#[test]
fn adapt_message_converts_value() {
  let adapt = AdaptMessage::<i32, NoStdToolbox>::new(5_u32, |value| Ok(value as i32));
  assert_eq!(adapt.execute(), AdapterOutcome::Converted(5));
}

#[test]
fn adapt_message_cannot_be_reused() {
  let adapt = AdaptMessage::<i32, NoStdToolbox>::new(7_u32, |value| Ok(value as i32));
  assert_eq!(adapt.execute(), AdapterOutcome::Converted(7));
  assert!(matches!(adapt.execute(), AdapterOutcome::Failure(_)));
}
