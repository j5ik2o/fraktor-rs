use super::StreamRefSinkLogic;
use crate::{
  DemandTracker, DynValue, SinkLogic, StreamError,
  r#impl::streamref::StreamRefHandoff,
  materialization::{Completion, StreamDone, StreamFuture},
  stream_ref::StreamRefSettings,
};

#[test]
fn awaiting_remote_subscription_requests_demand_after_subscribe() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff.clone());
  let mut demand = DemandTracker::new();

  logic.on_start(&mut demand).expect("start");
  assert!(!demand.has_demand());

  handoff.subscribe();
  assert!(logic.on_tick(&mut demand).expect("tick"));
  assert!(demand.has_demand());
}

#[test]
fn awaiting_remote_subscription_fails_after_configured_ticks() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(1));
  let mut demand = DemandTracker::new();

  let error = logic.on_tick(&mut demand).expect_err("subscription timeout");

  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}

#[test]
fn subscribed_sink_completes_materialized_completion() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.subscribe();
  let completion = StreamFuture::<StreamDone>::new();
  let mut logic = StreamRefSinkLogic::subscribed(handoff, Some(completion.clone()));

  logic.on_complete().expect("complete");

  assert!(matches!(completion.value(), Completion::Ready(Ok(_))));
}

#[test]
fn subscribed_sink_respects_configured_buffer_capacity() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.subscribe();
  let mut logic = StreamRefSinkLogic::subscribed(handoff, None);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_buffer_capacity(1));
  let mut demand = DemandTracker::new();

  let first: DynValue = Box::new(10_u32);
  logic.on_push(first, &mut demand).expect("first element fits capacity");
  let second: DynValue = Box::new(20_u32);
  let error = logic.on_push(second, &mut demand).expect_err("second element exceeds capacity");

  assert_eq!(error, StreamError::BufferOverflow);
}
