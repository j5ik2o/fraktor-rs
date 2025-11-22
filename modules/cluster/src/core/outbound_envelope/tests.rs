use super::OutboundEnvelope;

#[test]
fn new_sets_fields() {
  let envelope = OutboundEnvelope::new("pid-1".to_string(), vec![1, 2, 3]);

  assert_eq!(envelope.pid, "pid-1");
  assert_eq!(envelope.payload, vec![1, 2, 3]);
}
