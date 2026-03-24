use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{
  Completion, DynValue, KeepRight, SourceLogic, StageKind, StreamBufferConfig, StreamDone, StreamError,
  lifecycle::Stream,
  mat::{Materialized, Materializer},
  stage::{ActorSink, Source},
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), crate::core::StreamError> {
    Ok(())
  }

  fn materialize<Mat>(
    &mut self,
    graph: crate::core::mat::RunnableGraph<Mat>,
  ) -> Result<Materialized<Mat>, crate::core::StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = crate::core::lifecycle::StreamShared::new(stream);
    let handle = crate::core::lifecycle::StreamHandleImpl::new(crate::core::lifecycle::StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), crate::core::StreamError> {
    Ok(())
  }
}

fn drive_until_terminal<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..64 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
}

struct CancelTrackingSourceLogic {
  values:       VecDeque<u32>,
  pull_count:   ArcShared<SpinSyncMutex<u32>>,
  cancel_count: ArcShared<SpinSyncMutex<u32>>,
}

impl CancelTrackingSourceLogic {
  fn new(
    values: [u32; 3],
    pull_count: ArcShared<SpinSyncMutex<u32>>,
    cancel_count: ArcShared<SpinSyncMutex<u32>>,
  ) -> Self {
    Self { values: VecDeque::from(values), pull_count, cancel_count }
  }
}

impl SourceLogic for CancelTrackingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let mut pull_count = self.pull_count.lock();
    *pull_count = pull_count.saturating_add(1);
    Ok(self.values.pop_front().map(|value| Box::new(value) as DynValue))
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    let mut guard = self.cancel_count.lock();
    *guard = guard.saturating_add(1);
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BackpressureMessage {
  Init { ack: u8 },
  Element { ack: u8, value: u32 },
  Complete,
  Failure,
}

#[test]
fn actor_sink_actor_ref_should_complete_stream() {
  let forwarded = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let graph = Source::from_array([1_u32, 2_u32]).to_mat(
    ActorSink::actor_ref({
      let forwarded = forwarded.clone();
      move |value| {
        forwarded.lock().push(value);
      }
    }),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(forwarded.lock().as_slice(), &[1_u32, 2_u32]);
}

#[test]
fn actor_sink_actor_ref_should_not_cancel_upstream() {
  let pull_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let forwarded = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let source = Source::<u32, _>::from_logic(
    StageKind::Custom,
    CancelTrackingSourceLogic::new([1_u32, 2_u32, 3_u32], pull_count.clone(), cancel_count.clone()),
  );
  let graph = source.to_mat(
    ActorSink::actor_ref({
      let forwarded = forwarded.clone();
      move |value| {
        forwarded.lock().push(value);
      }
    }),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert!(*pull_count.lock() >= 3_u32);
  assert_eq!(*cancel_count.lock(), 0_u32);
  assert_eq!(forwarded.lock().as_slice(), &[1_u32, 2_u32, 3_u32]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_should_complete_stream() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<BackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([1_u8, 1_u8, 1_u8])));

  let graph = Source::from_array([1_u32, 2_u32]).to_mat(
    ActorSink::actor_ref_with_backpressure(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |ack, value| BackpressureMessage::Element { ack, value },
      |ack| BackpressureMessage::Init { ack },
      {
        let acks = acks.clone();
        move || acks.lock().pop_front()
      },
      1_u8,
      BackpressureMessage::Complete,
      |_error| BackpressureMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));
  assert_eq!(messages.lock().as_slice(), &[
    BackpressureMessage::Init { ack: 1_u8 },
    BackpressureMessage::Element { ack: 1_u8, value: 1_u32 },
    BackpressureMessage::Element { ack: 1_u8, value: 2_u32 },
    BackpressureMessage::Complete,
  ]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_should_pause_without_ack() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<BackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([1_u8])));

  let graph = Source::from_array([1_u32, 2_u32]).to_mat(
    ActorSink::actor_ref_with_backpressure(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |ack, value| BackpressureMessage::Element { ack, value },
      |ack| BackpressureMessage::Init { ack },
      {
        let acks = acks.clone();
        move || acks.lock().pop_front()
      },
      1_u8,
      BackpressureMessage::Complete,
      |_error| BackpressureMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert_eq!(materialized.materialized().poll(), Completion::Pending);
  assert!(!materialized.handle().state().is_terminal());
  assert_eq!(messages.lock().as_slice(), &[BackpressureMessage::Init { ack: 1_u8 }, BackpressureMessage::Element {
    ack:   1_u8,
    value: 1_u32,
  },]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_should_resume_after_delayed_ack() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<BackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([1_u8])));

  let graph = Source::from_array([1_u32, 2_u32]).to_mat(
    ActorSink::actor_ref_with_backpressure(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |ack, value| BackpressureMessage::Element { ack, value },
      |ack| BackpressureMessage::Init { ack },
      {
        let acks = acks.clone();
        move || acks.lock().pop_front()
      },
      1_u8,
      BackpressureMessage::Complete,
      |_error| BackpressureMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  drive_until_terminal(&materialized);

  assert_eq!(materialized.materialized().poll(), Completion::Pending);
  assert_eq!(messages.lock().as_slice(), &[BackpressureMessage::Init { ack: 1_u8 }, BackpressureMessage::Element {
    ack:   1_u8,
    value: 1_u32,
  },]);

  acks.lock().push_back(1_u8);
  acks.lock().push_back(1_u8);
  drive_until_terminal(&materialized);

  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));
  assert_eq!(messages.lock().as_slice(), &[
    BackpressureMessage::Init { ack: 1_u8 },
    BackpressureMessage::Element { ack: 1_u8, value: 1_u32 },
    BackpressureMessage::Element { ack: 1_u8, value: 2_u32 },
    BackpressureMessage::Complete,
  ]);
}

// --- actor_ref_with_backpressure_no_ack tests ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum NoAckMessage {
  Element(u32),
  Complete,
  Failure,
}

#[test]
fn actor_sink_no_ack_should_complete_stream() {
  // Given: a source with 2 elements and no-ack backpressure sink
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<NoAckMessage>::new()));

  let graph = Source::from_array([1_u32, 2_u32]).to_mat(
    ActorSink::actor_ref_with_backpressure_no_ack(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |value| NoAckMessage::Element(value),
      NoAckMessage::Complete,
      |_error| NoAckMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  // Then: stream completes and all elements are forwarded
  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));
  assert_eq!(messages.lock().as_slice(), &[
    NoAckMessage::Element(1_u32),
    NoAckMessage::Element(2_u32),
    NoAckMessage::Complete,
  ]);
}

#[test]
fn actor_sink_no_ack_should_forward_all_elements_without_explicit_ack() {
  // Given: a source with 3 elements and no-ack sink (no ack queue needed)
  let forwarded = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));

  let graph = Source::from_array([10_u32, 20_u32, 30_u32]).to_mat(
    ActorSink::actor_ref_with_backpressure_no_ack(
      {
        let forwarded = forwarded.clone();
        move |message: u32| {
          forwarded.lock().push(message);
        }
      },
      |value| value,
      0_u32,
      |_error| 0_u32,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  // Then: all 3 elements forwarded without pausing
  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));
  assert_eq!(forwarded.lock().as_slice(), &[10_u32, 20_u32, 30_u32]);
}

#[test]
fn actor_sink_no_ack_should_not_cancel_upstream() {
  // Given: a source with cancel tracking and no-ack sink
  let pull_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let source = Source::<u32, _>::from_logic(
    StageKind::Custom,
    CancelTrackingSourceLogic::new([1_u32, 2_u32, 3_u32], pull_count.clone(), cancel_count.clone()),
  );
  let graph = source.to_mat(
    ActorSink::actor_ref_with_backpressure_no_ack(|_message: u32| {}, |value| value, 0_u32, |_error| 0_u32),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  // Then: all elements pulled, no cancellation
  assert!(*pull_count.lock() >= 3_u32);
  assert_eq!(*cancel_count.lock(), 0_u32);
}
