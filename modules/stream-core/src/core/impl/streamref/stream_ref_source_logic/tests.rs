use super::StreamRefSourceLogic;
use crate::core::{SourceLogic, StreamError, r#impl::streamref::StreamRefHandoff, stream_ref::StreamRefSettings};

#[test]
fn awaiting_remote_subscription_fails_after_configured_ticks() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSourceLogic::awaiting_remote_subscription(handoff);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(1));

  let error = logic.pull().expect_err("subscription timeout");

  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}

#[test]
fn subscribed_source_polls_values_until_completion() {
  let handoff = StreamRefHandoff::new();
  handoff.subscribe();
  handoff.offer(42_u32).expect("offer");
  handoff.complete();
  let mut logic = StreamRefSourceLogic::subscribed(handoff);

  assert!(logic.pull().expect("value").is_some());
  assert!(logic.pull().expect("complete").is_none());
}
