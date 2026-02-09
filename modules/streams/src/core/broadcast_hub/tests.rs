use crate::core::BroadcastHub;

#[test]
fn broadcast_hub_delivers_to_all_subscribers() {
  let mut hub = BroadcastHub::new();
  let left = hub.subscribe();
  let right = hub.subscribe();

  hub.publish(10_u32);
  hub.publish(20_u32);

  assert_eq!(hub.poll(left), Some(10_u32));
  assert_eq!(hub.poll(left), Some(20_u32));
  assert_eq!(hub.poll(right), Some(10_u32));
  assert_eq!(hub.poll(right), Some(20_u32));
}
