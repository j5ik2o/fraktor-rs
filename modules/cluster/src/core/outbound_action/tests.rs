use crate::core::outbound_envelope::OutboundEnvelope;

use super::OutboundAction;

#[test]
fn immediate_carries_envelope() {
  let envelope = OutboundEnvelope::new("pid-1".to_string(), vec![1]);
  let action = OutboundAction::Immediate { envelope };

  match action {
    | OutboundAction::Immediate { envelope } => {
      assert_eq!(envelope.pid, "pid-1");
    },
    | _ => panic!("unexpected variant"),
  }
}
