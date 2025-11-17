use crate::core::{error::SendError, messaging::AnyMessage};

#[test]
fn retains_message() {
  let message: AnyMessage = AnyMessage::new(42_u32);
  let error: SendError = SendError::full(message.clone());
  assert_eq!(error.message().payload().downcast_ref::<u32>(), Some(&42));
  let recovered = error.into_message();
  assert_eq!(recovered.payload().downcast_ref::<u32>(), Some(&42));
}
