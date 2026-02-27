use super::RemotingLifecycleEvent;
use crate::core::event::stream::CorrelationId;

#[test]
fn preserves_existing_variants() {
  let event = RemotingLifecycleEvent::Gated {
    authority:      "127.0.0.1:2559".into(),
    correlation_id: CorrelationId::from_u128(33),
  };
  assert!(matches!(
    event,
    RemotingLifecycleEvent::Gated {
      authority,
      correlation_id
    } if authority == "127.0.0.1:2559" && correlation_id == CorrelationId::from_u128(33)
  ));
}
