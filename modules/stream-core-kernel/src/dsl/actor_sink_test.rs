use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{
  DynValue, SourceLogic, StageKind, StreamError,
  dsl::{ActorSink, Source},
  r#impl::{
    fusing::StreamBufferConfig,
    materialization::{Stream, StreamShared},
  },
  materialization::{
    Completion, DriveOutcome, KeepRight, Materialized, Materializer, RunnableGraph, StreamDone, StreamFuture,
    StreamNotUsed,
  },
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), crate::r#impl::StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, crate::r#impl::StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let stream = StreamShared::new(stream);
    Ok(Materialized::new(stream, materialized))
  }

  fn shutdown(&mut self) -> Result<(), crate::r#impl::StreamError> {
    Ok(())
  }
}

fn drive_until_terminal<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..64 {
    match materialized.stream().drive() {
      | DriveOutcome::Progressed | DriveOutcome::Idle => {},
    }
    if materialized.stream().state().is_terminal() {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum AnyAckBackpressureMessage {
  Init,
  Element { value: u32 },
  Complete,
  Failure,
}

#[test]
fn actor_sink_actor_ref_should_complete_stream() {
  let forwarded = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let graph = Source::from_array([1_u32, 2_u32]).into_mat(
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

  assert_eq!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone::new())));
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
  let graph = source.into_mat(
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
fn actor_sink_actor_ref_with_result_should_fail_stream_when_callback_fails() {
  let graph = Source::from_array([1_u32, 2_u32]).into_mat(
    ActorSink::actor_ref_with_result(|value| if value == 2 { Err(StreamError::Failed) } else { Ok(()) }),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert_eq!(materialized.materialized().value(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn actor_sink_actor_ref_with_result_should_complete_stream_when_callback_succeeds() {
  let graph = Source::from_array([1_u32, 2_u32]).into_mat(ActorSink::actor_ref_with_result(|_value| Ok(())), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  assert_eq!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn actor_sink_actor_ref_with_backpressure_should_complete_stream() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<BackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([1_u8, 1_u8, 1_u8])));

  let graph: RunnableGraph<StreamFuture<StreamDone>> = Source::from_array([1_u32, 2_u32]).into_mat(
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

  assert!(matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));
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

  let graph: RunnableGraph<StreamFuture<StreamDone>> = Source::from_array([1_u32, 2_u32]).into_mat(
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

  assert_eq!(materialized.materialized().value(), Completion::Pending);
  assert!(!materialized.stream().state().is_terminal());
  assert_eq!(messages.lock().as_slice(), &[BackpressureMessage::Init { ack: 1_u8 }, BackpressureMessage::Element {
    ack:   1_u8,
    value: 1_u32,
  },]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_should_resume_after_delayed_ack() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<BackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([1_u8])));

  let graph = Source::from_array([1_u32, 2_u32]).into_mat(
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

  assert_eq!(materialized.materialized().value(), Completion::Pending);
  assert_eq!(messages.lock().as_slice(), &[BackpressureMessage::Init { ack: 1_u8 }, BackpressureMessage::Element {
    ack:   1_u8,
    value: 1_u32,
  },]);

  acks.lock().push_back(1_u8);
  acks.lock().push_back(1_u8);
  drive_until_terminal(&materialized);

  assert!(matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));
  assert_eq!(messages.lock().as_slice(), &[
    BackpressureMessage::Init { ack: 1_u8 },
    BackpressureMessage::Element { ack: 1_u8, value: 1_u32 },
    BackpressureMessage::Element { ack: 1_u8, value: 2_u32 },
    BackpressureMessage::Complete,
  ]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_any_ack_should_accept_different_ack_values() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<AnyAckBackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([7_u8, 8_u8, 9_u8])));

  // Given: ack 値の一致判定を持たない backpressure sink
  let graph: RunnableGraph<StreamNotUsed> = Source::from_array([1_u32, 2_u32]).into_mat(
    ActorSink::actor_ref_with_backpressure_any_ack(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |value| AnyAckBackpressureMessage::Element { value },
      || AnyAckBackpressureMessage::Init,
      {
        let acks = acks.clone();
        move || acks.lock().pop_front()
      },
      AnyAckBackpressureMessage::Complete,
      |_error| AnyAckBackpressureMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;

  // When: init と各要素に異なる ack 値を返す
  let materialized = graph.run(&mut materializer).expect("run");
  drive_until_terminal(&materialized);

  // Then: Some(_) を ack として扱い、全要素を流して完了する
  assert!(materialized.stream().state().is_terminal());
  assert_eq!(*materialized.materialized(), StreamNotUsed::new());
  assert_eq!(messages.lock().as_slice(), &[
    AnyAckBackpressureMessage::Init,
    AnyAckBackpressureMessage::Element { value: 1_u32 },
    AnyAckBackpressureMessage::Element { value: 2_u32 },
    AnyAckBackpressureMessage::Complete,
  ]);
}

#[test]
fn actor_sink_actor_ref_with_backpressure_any_ack_should_pause_until_ack_is_available() {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::<AnyAckBackpressureMessage>::new()));
  let acks = ArcShared::new(SpinSyncMutex::new(VecDeque::from([7_u8])));

  // Given: init ack だけが先に到達している any-ack backpressure sink
  let graph: RunnableGraph<StreamNotUsed> = Source::from_array([1_u32, 2_u32]).into_mat(
    ActorSink::actor_ref_with_backpressure_any_ack(
      {
        let messages = messages.clone();
        move |message| {
          messages.lock().push(message);
        }
      },
      |value| AnyAckBackpressureMessage::Element { value },
      || AnyAckBackpressureMessage::Init,
      {
        let acks = acks.clone();
        move || acks.lock().pop_front()
      },
      AnyAckBackpressureMessage::Complete,
      |_error| AnyAckBackpressureMessage::Failure,
    ),
    KeepRight,
  );
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("run");

  // When: 1 要素目送信後の ack がまだない状態まで進める
  drive_until_terminal(&materialized);

  // Then: stream は pending のまま追加 demand を出さない
  assert!(!materialized.stream().state().is_terminal());
  assert_eq!(*materialized.materialized(), StreamNotUsed::new());
  assert_eq!(messages.lock().as_slice(), &[AnyAckBackpressureMessage::Init, AnyAckBackpressureMessage::Element {
    value: 1_u32,
  },]);

  // When: 値の異なる ack を後から供給する
  acks.lock().push_back(8_u8);
  acks.lock().push_back(9_u8);
  drive_until_terminal(&materialized);

  // Then: ack 値を比較せず再開して完了する
  assert!(materialized.stream().state().is_terminal());
  assert_eq!(*materialized.materialized(), StreamNotUsed::new());
  assert_eq!(messages.lock().as_slice(), &[
    AnyAckBackpressureMessage::Init,
    AnyAckBackpressureMessage::Element { value: 1_u32 },
    AnyAckBackpressureMessage::Element { value: 2_u32 },
    AnyAckBackpressureMessage::Complete,
  ]);
}
