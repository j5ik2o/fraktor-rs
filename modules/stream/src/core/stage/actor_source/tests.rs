use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{
  Completion, KeepBoth, OverflowStrategy, QueueOfferResult, StreamBufferConfig, StreamError,
  lifecycle::Stream,
  mat::{Materialized, Materializer, RunnableGraph},
  stage::{ActorSource, Sink},
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = crate::core::lifecycle::StreamShared::new(stream);
    let handle = crate::core::lifecycle::StreamHandleImpl::new(crate::core::lifecycle::StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

fn drive_until_terminal<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..256 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
  }
}

// --- ActorSource::actor_ref ---

#[test]
fn actor_source_actor_ref_should_emit_told_values() {
  // Given: a source created with actor_ref
  let source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: telling values and then completing
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();
  assert_eq!(source_ref.tell(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(source_ref.tell(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(source_ref.tell(3_u32), QueueOfferResult::Enqueued);
  source_ref.complete();

  // Then: drive stream to completion and collect values
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 2_u32, 3_u32])));
}

#[test]
fn actor_source_actor_ref_should_complete_with_empty_output_when_no_values_told() {
  // Given: a source created with actor_ref
  let source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: completing immediately without telling any values
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();
  source_ref.complete();

  // Then: stream completes with empty output
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(Vec::<u32>::new())));
}

#[test]
fn actor_source_actor_ref_should_respect_overflow_strategy() {
  // Given: a source with buffer size 2 and Fail overflow
  let source = ActorSource::actor_ref::<u32>(2, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: telling more values than buffer can hold
  let mut source_ref = materialized.materialized().0.clone();
  assert_eq!(source_ref.tell(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(source_ref.tell(2_u32), QueueOfferResult::Enqueued);

  // Then: third tell fails with BufferOverflow
  let result = source_ref.tell(3_u32);
  assert_eq!(result, QueueOfferResult::Failure(StreamError::BufferOverflow));
}

#[test]
fn actor_source_actor_ref_should_use_drop_head_overflow() {
  // Given: a source with buffer size 2 and DropHead overflow
  let source = ActorSource::actor_ref::<u32>(2, OverflowStrategy::DropHead);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: telling 3 values into buffer of size 2 with DropHead, then completing
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();
  let _ = source_ref.tell(1_u32);
  let _ = source_ref.tell(2_u32);
  let _ = source_ref.tell(3_u32);
  source_ref.complete();

  // Then: stream emits values 2 and 3 (head was dropped)
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![2_u32, 3_u32])));
}

#[test]
fn actor_source_actor_ref_should_reject_tell_after_complete() {
  // Given: a completed source
  let source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  let mut source_ref = materialized.materialized().0.clone();
  source_ref.complete();

  // When: telling after completion
  let result = source_ref.tell(1_u32);

  // Then: QueueClosed is returned
  assert_eq!(result, QueueOfferResult::QueueClosed);
}

#[test]
fn actor_source_actor_ref_should_reject_tell_after_fail() {
  // Given: a failed source
  let source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  let mut source_ref = materialized.materialized().0.clone();
  source_ref.fail(StreamError::Failed);

  // When: telling after failure
  let result = source_ref.tell(1_u32);

  // Then: Failure is returned
  assert_eq!(result, QueueOfferResult::Failure(StreamError::Failed));
}

#[test]
fn actor_source_actor_ref_materialized_ref_is_clone() {
  // Given: a source's materialized ActorSourceRef
  let source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Fail);
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();

  // When: cloning the ref and telling via the clone
  let mut cloned = source_ref.clone();
  let _ = cloned.tell(10_u32);
  source_ref.complete();

  // Then: the value appears in stream output
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![10_u32])));
}

#[test]
#[should_panic(expected = "Backpressure")]
fn actor_source_actor_ref_should_reject_backpressure_strategy() {
  // Given/When: creating a source with Backpressure overflow strategy
  // Then: panic (Pekko contract: "Backpressure overflowStrategy not supported")
  let _source = ActorSource::actor_ref::<u32>(4, OverflowStrategy::Backpressure);
}

// --- ActorSource::actor_ref_with_backpressure ---

#[test]
fn actor_source_actor_ref_with_backpressure_should_emit_told_values() {
  // Given: a source with backpressure semantics
  let acks = ArcShared::new(SpinSyncMutex::new(alloc::collections::VecDeque::<u8>::new()));
  let source = ActorSource::actor_ref_with_backpressure::<u32, u8, _>(1_u8, {
    let acks = acks.clone();
    move || acks.lock().pop_front()
  });
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: telling values with acks between each
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();
  let _ = source_ref.tell(1_u32);
  acks.lock().push_back(1_u8);
  let _ = source_ref.tell(2_u32);
  acks.lock().push_back(1_u8);
  source_ref.complete();
  acks.lock().push_back(1_u8);

  // Then: drive stream to completion and collect values
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 2_u32])));
}

#[test]
fn actor_source_actor_ref_with_backpressure_should_complete_with_empty_output() {
  // Given: a source with backpressure semantics
  let acks = ArcShared::new(SpinSyncMutex::new(alloc::collections::VecDeque::<u8>::new()));
  let source = ActorSource::actor_ref_with_backpressure::<u32, u8, _>(1_u8, {
    let acks = acks.clone();
    move || acks.lock().pop_front()
  });
  let graph = source.to_mat(Sink::<u32, crate::core::StreamCompletion<Vec<u32>>>::collect(), KeepBoth);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: completing immediately without telling any values
  let mut source_ref = materialized.materialized().0.clone();
  let completion = materialized.materialized().1.clone();
  source_ref.complete();
  acks.lock().push_back(1_u8);

  // Then: stream completes with empty output
  drive_until_terminal(&materialized);
  assert_eq!(completion.poll(), Completion::Ready(Ok(Vec::<u32>::new())));
}
