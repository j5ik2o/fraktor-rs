use crate::core::BroadcastHub;

#[test]
fn broadcast_hub_delivers_to_all_subscribers() {
  let hub = BroadcastHub::new();
  let left = hub.subscribe();
  let right = hub.subscribe();

  hub.publish(10_u32);
  hub.publish(20_u32);

  assert_eq!(hub.poll(left), Some(10_u32));
  assert_eq!(hub.poll(left), Some(20_u32));
  assert_eq!(hub.poll(right), Some(10_u32));
  assert_eq!(hub.poll(right), Some(20_u32));
}

#[test]
fn broadcast_hub_source_for_drains_subscriber_queue() {
  let hub = BroadcastHub::new();
  let left = hub.subscribe();
  let _right = hub.subscribe();
  hub.publish(1_u32);
  hub.publish(2_u32);

  let values = hub.source_for(left).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}
