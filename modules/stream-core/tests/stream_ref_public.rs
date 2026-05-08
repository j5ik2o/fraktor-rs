use core::time::Duration;
use std::time::Instant;

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  StreamError,
  dsl::{Sink, Source, StreamRefs},
  materialization::{
    ActorMaterializer, ActorMaterializerConfig, Completion, KeepBoth, KeepRight, StreamFuture, StreamNotUsed,
  },
  stream_ref::{SinkRef, SourceRef, StreamRefSettings},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_from_props(&props, config).expect("system should build")
}

fn build_materializer() -> ActorMaterializer {
  let config = ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1));
  build_materializer_with_config(config)
}

fn build_materializer_with_config(config: ActorMaterializerConfig) -> ActorMaterializer {
  let mut materializer = ActorMaterializer::new(build_system(), config);
  materializer.start().expect("materializer start");
  materializer
}

fn poll_completion<T>(completion: &StreamFuture<T>) -> Result<T, StreamError>
where
  T: Clone, {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if let Completion::Ready(result) = completion.value() {
      return result;
    }
    std::thread::yield_now();
  }
  panic!("stream did not complete");
}

fn collect_sink() -> Sink<u32, StreamFuture<Vec<u32>>> {
  Sink::fold(Vec::<u32>::new(), |mut values, value| {
    values.push(value);
    values
  })
}

#[test]
fn source_ref_materialized_from_sink_replays_upstream_to_remote_source_once() {
  // Given: SourceRef を materialize する StreamRefs.source_ref sink
  let mut materializer = build_materializer();
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(StreamRefs::source_ref::<u32>(), KeepRight);

  // When: materialized SourceRef を Source に変換して反対側で消費する
  let source_ref: SourceRef<u32> = graph.run(&mut materializer).expect("materialize source ref").into_materialized();
  let remote_source = source_ref.into_source();
  let received = remote_source.run_with(collect_sink(), &mut materializer).expect("run remote source");

  // Then: upstream の要素順序と完了が remote Source 側へ伝播する
  assert_eq!(poll_completion(received.materialized()).expect("remote source completion"), vec![1_u32, 2, 3]);
}

#[test]
fn sink_ref_materialized_from_source_accepts_remote_elements_and_completes_local_source() {
  // Given: SinkRef を materialize する StreamRefs.sink_ref source
  let mut materializer = build_materializer();
  let graph = StreamRefs::sink_ref::<u32>().into_mat(collect_sink(), KeepBoth);

  // When: materialized SinkRef を Sink に変換し、反対側から要素を流す
  let materialized = graph.run(&mut materializer).expect("materialize sink ref");
  let (sink_ref, received): (SinkRef<u32>, StreamFuture<Vec<u32>>) = materialized.into_materialized();
  let remote_done =
    Source::from_array([10_u32, 20, 30]).run_with(sink_ref.into_sink(), &mut materializer).expect("run remote sink");

  // Then: remote Sink への入力は local Source 側へ順序どおり流れ、両側が完了する
  assert_eq!(remote_done.materialized(), &StreamNotUsed::new());
  assert_eq!(poll_completion(&received).expect("local source completion"), vec![10_u32, 20, 30]);
}

#[test]
fn source_ref_and_sink_ref_are_one_shot_owned_conversions() {
  // Given/When: public conversion method を関数ポインタとして取り出す

  // Then: into_source / into_sink は self を消費する API として型で one-shot を表す
  let _source_into_source: fn(SourceRef<u32>) -> Source<u32, StreamNotUsed> = SourceRef::into_source;
  let _sink_into_sink: fn(SinkRef<u32>) -> Sink<u32, StreamNotUsed> = SinkRef::into_sink;
}

#[test]
fn sink_ref_source_fails_when_remote_sink_never_subscribes_before_timeout() {
  // Given: subscription timeout を 1 tick に縮めた materializer
  let settings = StreamRefSettings::new().with_subscription_timeout_ticks(1);
  let config =
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)).with_stream_ref_settings(settings);
  let mut materializer = build_materializer_with_config(config);
  let graph = StreamRefs::sink_ref::<u32>().into_mat(collect_sink(), KeepBoth);

  // When: SinkRef を materialize するが、反対側の Sink として使用しない
  let materialized = graph.run(&mut materializer).expect("materialize sink ref");
  let (_sink_ref, received): (SinkRef<u32>, StreamFuture<Vec<u32>>) = materialized.into_materialized();

  // Then: materializer の StreamRefSettings から来た timeout tick により local Source が失敗する
  let error = poll_completion(&received).expect_err("subscription timeout");
  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}
