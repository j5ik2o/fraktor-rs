use crate::core::{
  RetryFlow, StreamNotUsed,
  stage::{FlowWithContext, Source, flow::Flow},
};

#[test]
fn retry_flow_passes_through_when_no_retry_needed() {
  let inner_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_function(|x: u32| x.saturating_mul(2));
  let retry_flow = RetryFlow::with_backoff(1, 10, 0, 3, inner_flow, |_input: &u32, _output: &u32| None::<u32>);
  let values = Source::from_iterator(vec![1_u32, 2, 3]).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![2_u32, 4, 6]);
}

#[test]
fn retry_flow_retries_and_succeeds() {
  let attempt = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
  let attempt_clone = attempt.clone();

  let inner_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_function(move |x: u32| {
    let count = attempt_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if count == 0 { 0_u32 } else { x.saturating_mul(10) }
  });

  let retry_flow = RetryFlow::with_backoff(
    0,
    0,
    0,
    3,
    inner_flow,
    |_input: &u32, output: &u32| {
      if *output == 0 { Some(42_u32) } else { None }
    },
  );

  let values = Source::single(42_u32).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![420_u32]);
}

#[test]
fn retry_flow_exhausts_max_retries_and_passes_through() {
  let inner_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_function(|_x: u32| 0_u32);

  let retry_flow = RetryFlow::with_backoff(
    0,
    0,
    0,
    2,
    inner_flow,
    |input: &u32, output: &u32| {
      if *output == 0 { Some(*input) } else { None }
    },
  );

  let values = Source::single(5_u32).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![0_u32]);
}

#[test]
fn retry_flow_with_zero_max_retries_never_retries() {
  let inner_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_function(|_x: u32| 0_u32);

  let retry_flow = RetryFlow::with_backoff(
    1,
    10,
    0,
    0,
    inner_flow,
    |input: &u32, output: &u32| {
      if *output == 0 { Some(*input) } else { None }
    },
  );

  let values = Source::single(5_u32).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![0_u32]);
}

#[test]
fn retry_flow_preserves_multiple_pending_retries_in_order() {
  let inner_flow: Flow<u32, u32, StreamNotUsed> =
    Flow::from_function(|x: u32| x).map_concat(|x: u32| alloc::vec![x.saturating_add(1), x.saturating_add(2)]);

  let retry_flow =
    RetryFlow::with_backoff(
      0,
      0,
      0,
      4,
      inner_flow,
      |input: &u32, output: &u32| {
        if *input < 10 { Some(output.saturating_add(100)) } else { None }
      },
    );

  let values = Source::single(1_u32).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![103_u32, 104_u32, 104_u32, 105_u32]);
}

#[test]
fn retry_flow_accepts_remaining_outputs_after_scheduling_retry() {
  let inner_flow: Flow<u32, u32, StreamNotUsed> =
    Flow::from_function(|x: u32| x).map_concat(|x: u32| alloc::vec![x.saturating_mul(2), 0_u32]);

  let retry_flow = RetryFlow::with_backoff(0, 0, 0, 3, inner_flow, |input: &u32, output: &u32| {
    if *input < 10 && *output == 0 { Some(input.saturating_add(100)) } else { None }
  });

  let values = Source::single(1_u32).via(retry_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![2_u32, 202_u32, 0_u32]);
}

#[test]
fn retry_flow_with_context_passes_through_when_no_retry_needed() {
  let inner_flow: Flow<(String, u32), (String, u32), StreamNotUsed> =
    Flow::from_function(|(ctx, x): (String, u32)| (ctx, x.saturating_mul(2)));
  let fwc = FlowWithContext::from_flow(inner_flow);

  let retry_fwc =
    RetryFlow::with_backoff_and_context(1, 10, 0, 3, fwc, |_input: &(String, u32), _output: &(String, u32)| None);

  let values =
    Source::from_iterator(vec![("a".to_string(), 1_u32), ("b".to_string(), 2_u32), ("c".to_string(), 3_u32)])
      .via(retry_fwc.as_flow())
      .collect_values()
      .expect("collect_values");
  assert_eq!(values, vec![("a".to_string(), 2_u32), ("b".to_string(), 4_u32), ("c".to_string(), 6_u32),]);
}

#[test]
fn retry_flow_with_context_retries_preserving_context() {
  let attempt = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
  let attempt_clone = attempt.clone();

  let inner_flow: Flow<(String, u32), (String, u32), StreamNotUsed> =
    Flow::from_function(move |(ctx, x): (String, u32)| {
      let count = attempt_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      if count == 0 { (ctx, 0_u32) } else { (ctx, x.saturating_mul(10)) }
    });
  let fwc = FlowWithContext::from_flow(inner_flow);

  let retry_fwc = RetryFlow::with_backoff_and_context(
    0,
    0,
    0,
    3,
    fwc,
    |input: &(String, u32), output: &(String, u32)| {
      if output.1 == 0 { Some(input.clone()) } else { None }
    },
  );

  let values =
    Source::single(("ctx-val".to_string(), 42_u32)).via(retry_fwc.as_flow()).collect_values().expect("collect_values");
  assert_eq!(values, vec![("ctx-val".to_string(), 420_u32)]);
}

#[test]
fn retry_flow_with_context_exhausts_retries() {
  let inner_flow: Flow<(i32, u32), (i32, u32), StreamNotUsed> =
    Flow::from_function(|(ctx, _x): (i32, u32)| (ctx, 0_u32));
  let fwc = FlowWithContext::from_flow(inner_flow);

  let retry_fwc = RetryFlow::with_backoff_and_context(
    0,
    0,
    0,
    2,
    fwc,
    |input: &(i32, u32), output: &(i32, u32)| {
      if output.1 == 0 { Some(*input) } else { None }
    },
  );

  let values = Source::single((99_i32, 5_u32)).via(retry_fwc.as_flow()).collect_values().expect("collect_values");
  assert_eq!(values, vec![(99_i32, 0_u32)]);
}
