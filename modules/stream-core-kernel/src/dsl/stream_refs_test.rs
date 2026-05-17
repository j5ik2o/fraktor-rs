use core::{hint, time::Duration};
use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{actor_ref_provider::LocalActorRefProviderInstaller, scheduler::SchedulerConfig},
  system::ActorSystem,
};

use super::StreamRefs;
use crate::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source},
  materialization::{
    ActorMaterializer, ActorMaterializerConfig, Completion, DriveOutcome, KeepBoth, KeepLeft, KeepRight, Materialized,
  },
  stage::StageKind,
  stream_ref::{SinkRef, SourceRef, StreamRefResolver},
};

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| {
    config.with_scheduler_config(scheduler).with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
  })
}

fn build_materializer(system: ActorSystem) -> ActorMaterializer {
  let config = ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1));
  let mut materializer = ActorMaterializer::new(system, config);
  materializer.start().expect("materializer start");
  materializer
}

fn drive_pair_until<Left, Right, F>(left: &Materialized<Left>, right: &Materialized<Right>, is_ready: F)
where
  F: Fn() -> bool, {
  for _ in 0..4096 {
    if is_ready() {
      return;
    }
    let left_progressed = if left.stream().state().is_terminal() {
      false
    } else {
      matches!(left.stream().drive(), DriveOutcome::Progressed)
    };
    let right_progressed = if right.stream().state().is_terminal() {
      false
    } else {
      matches!(right.stream().drive(), DriveOutcome::Progressed)
    };
    if left.stream().state().is_terminal() && right.stream().state().is_terminal() {
      return;
    }
    if !(left_progressed || right_progressed) {
      hint::spin_loop();
    }
  }
  panic!(
    "stream refs did not complete before timeout: left={:?} right={:?}",
    left.stream().state(),
    right.stream().state()
  );
}

struct PendingSourceLogic;

impl SourceLogic for PendingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }
}

#[test]
fn source_ref_returns_sink_materializing_source_ref() {
  let _sink: Sink<u32, SourceRef<u32>> = StreamRefs::source_ref();
}

#[test]
fn sink_ref_returns_source_materializing_sink_ref() {
  let _source: Source<u32, SinkRef<u32>> = StreamRefs::sink_ref();
}

#[test]
fn drive_pair_until_covers_ready_and_timeout_paths() {
  let mut materializer = build_materializer(build_system());
  let left = Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic)
    .into_mat(Sink::ignore(), KeepRight)
    .run(&mut materializer)
    .expect("left pending stream");
  let right = Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic)
    .into_mat(Sink::ignore(), KeepRight)
    .run(&mut materializer)
    .expect("right pending stream");

  drive_pair_until(&left, &right, || true);
  let result = catch_unwind(AssertUnwindSafe(|| drive_pair_until(&left, &right, || false)));

  assert!(result.is_err());
}

#[test]
fn source_ref_materialization_installs_actor_backed_endpoint_path() {
  let mut materializer = build_materializer(build_system());
  let graph = Source::from_array([1_u32]).into_mat(StreamRefs::source_ref::<u32>(), KeepRight);

  let source_ref: SourceRef<u32> = graph.run(&mut materializer).expect("materialize SourceRef").into_materialized();
  let canonical = source_ref.canonical_actor_path().expect("canonical actor path");

  assert!(canonical.starts_with("fraktor://"));
  assert!(canonical.contains("/temp/"));
}

#[test]
fn sink_ref_materialization_installs_actor_backed_endpoint_path() {
  let mut materializer = build_materializer(build_system());
  let graph = StreamRefs::sink_ref::<u32>().into_mat(Sink::ignore(), KeepLeft);

  let sink_ref: SinkRef<u32> = graph.run(&mut materializer).expect("materialize SinkRef").into_materialized();
  let canonical = sink_ref.canonical_actor_path().expect("canonical actor path");

  assert!(canonical.starts_with("fraktor://"));
  assert!(canonical.contains("/temp/"));
}

#[test]
fn source_ref_serialized_format_resolves_to_source_ref_through_provider_dispatch() {
  let system = build_system();
  let resolver = StreamRefResolver::new(system.clone());
  let mut materializer = build_materializer(system);
  let graph = Source::from_array([1_u32]).into_mat(StreamRefs::source_ref::<u32>(), KeepRight);
  let source_ref: SourceRef<u32> = graph.run(&mut materializer).expect("materialize SourceRef").into_materialized();

  let serialized = resolver.source_ref_to_format(&source_ref).expect("source ref format");
  let resolved = resolver.resolve_source_ref::<u32>(&serialized).expect("resolve SourceRef");

  assert_eq!(resolver.source_ref_to_format(&resolved).expect("resolved source ref format"), serialized);
}

#[test]
fn source_ref_round_trip_carries_elements_locally() {
  let system = build_system();
  let resolver = StreamRefResolver::new(system.clone());
  let mut materializer = build_materializer(system);
  let producer = Source::from_array([1_i32, 2, 3]).into_mat(StreamRefs::source_ref::<i32>(), KeepRight);
  let producer_materialized = producer.run(&mut materializer).expect("materialize SourceRef producer");
  let serialized = resolver.source_ref_to_format(producer_materialized.materialized()).expect("source ref format");
  let resolved = resolver.resolve_source_ref::<i32>(&serialized).expect("resolve SourceRef");
  let consumer_materialized = resolved
    .into_source()
    .run_with(Sink::<i32, _>::collect(), &mut materializer)
    .expect("materialize resolved SourceRef consumer");
  let completion = consumer_materialized.materialized().clone();

  drive_pair_until(&producer_materialized, &consumer_materialized, || completion.is_ready());

  assert_eq!(completion.value(), Completion::Ready(Ok(alloc::vec![1_i32, 2, 3])));
}

#[test]
fn sink_ref_serialized_format_resolves_to_sink_ref_through_provider_dispatch() {
  let system = build_system();
  let resolver = StreamRefResolver::new(system.clone());
  let mut materializer = build_materializer(system);
  let graph = StreamRefs::sink_ref::<u32>().into_mat(Sink::ignore(), KeepLeft);
  let sink_ref: SinkRef<u32> = graph.run(&mut materializer).expect("materialize SinkRef").into_materialized();

  let serialized = resolver.sink_ref_to_format(&sink_ref).expect("sink ref format");
  let resolved = resolver.resolve_sink_ref::<u32>(&serialized).expect("resolve SinkRef");

  assert_eq!(resolver.sink_ref_to_format(&resolved).expect("resolved sink ref format"), serialized);
}

#[test]
fn sink_ref_round_trip_carries_elements_locally() {
  let system = build_system();
  let resolver = StreamRefResolver::new(system.clone());
  let mut materializer = build_materializer(system);
  let consumer = StreamRefs::sink_ref::<i32>().into_mat(Sink::<i32, _>::collect(), KeepBoth);
  let consumer_materialized = consumer.run(&mut materializer).expect("materialize SinkRef consumer");
  let serialized = resolver.sink_ref_to_format(&consumer_materialized.materialized().0).expect("sink ref format");
  let resolved = resolver.resolve_sink_ref::<i32>(&serialized).expect("resolve SinkRef");
  let producer_materialized = Source::from_array([10_i32, 20, 30])
    .run_with(resolved.into_sink(), &mut materializer)
    .expect("materialize resolved SinkRef producer");
  let completion = consumer_materialized.materialized().1.clone();

  drive_pair_until(&consumer_materialized, &producer_materialized, || completion.is_ready());

  assert_eq!(completion.value(), Completion::Ready(Ok(alloc::vec![10_i32, 20, 30])));
}
