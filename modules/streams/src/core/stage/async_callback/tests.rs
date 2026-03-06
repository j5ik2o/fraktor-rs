use crate::core::stage::AsyncCallback;

#[test]
fn async_callback_should_queue_values() {
  let callback = AsyncCallback::new();

  callback.invoke(1_u32);
  callback.invoke(2_u32);

  assert_eq!(callback.len(), 2);
  assert_eq!(callback.drain(), vec![1_u32, 2_u32]);
  assert!(callback.is_empty());
}

#[test]
fn async_callback_should_share_state_across_clones() {
  let callback = AsyncCallback::new();
  let clone = callback.clone();

  callback.invoke(7_u32);
  assert_eq!(clone.drain(), vec![7_u32]);
  assert!(callback.is_empty());
}
