use crate::core::{outbound_envelope::OutboundEnvelope, outbound_event::OutboundEvent};

#[test]
fn dropped_event_keeps_envelope() {
  let event = OutboundEvent::DroppedOldest {
    dropped: OutboundEnvelope::new("pid-1".to_string(), vec![7]),
    reason:  "queue overflow".to_string(),
  };

  match event {
    | OutboundEvent::DroppedOldest { dropped, reason } => {
      assert_eq!(dropped.pid, "pid-1");
      assert_eq!(reason, "queue overflow");
    },
    | _ => panic!("unexpected variant"),
  }
}
