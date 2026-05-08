mod support;
use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  dsl::{CoupledTerminationFlow, Flow, Sink, Source},
  materialization::{
    ActorMaterializer, ActorMaterializerConfig, Completion, KeepBoth, KeepLeft, KeepRight, StreamFuture, StreamNotUsed,
  },
};
use support::RunWithCollectSink;

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

fn wait_until_ready<T>(completion: &StreamFuture<T>)
where
  T: Clone, {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if matches!(completion.value(), Completion::Ready(_)) {
      return;
    }
    thread::sleep(Duration::from_millis(10));
  }
  panic!("stream completion did not become ready before deadline");
}

fn wait_until_true(flag: &AtomicBool) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if flag.load(Ordering::SeqCst) {
      return;
    }
    thread::sleep(Duration::from_millis(10));
  }
  panic!("flag did not become true before deadline");
}

#[test]
fn coupled_termination_flow_from_sink_and_source_returns_flow_with_stream_not_used_mat() {
  let sink = Sink::<u32, _>::ignore();
  let source = Source::single(99_u32);

  let flow: Flow<u32, u32, StreamNotUsed> = CoupledTerminationFlow::from_sink_and_source(sink, source);
  let graph = Source::single(7_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &StreamNotUsed::new());
}

#[test]
fn coupled_termination_flow_from_sink_and_source_emits_elements_from_embedded_source() {
  let sink = Sink::<u32, _>::ignore();
  let source = Source::single(42_u32);
  let flow = CoupledTerminationFlow::from_sink_and_source(sink, source);

  let values = Source::single(7_u32).via(flow).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![42_u32]);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_left_keeps_sink_materialized_value() {
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepLeft);
  let graph = Source::single(7_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &99_i32);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_right_keeps_source_materialized_value() {
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);
  let graph = Source::single(7_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &true);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_both_keeps_both_materialized_values() {
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepBoth);
  let graph = Source::single(7_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &(99_i32, true));
}

#[test]
fn coupled_termination_flow_completes_wrapped_sink_when_embedded_source_finishes() {
  let sink_completed = Arc::new(AtomicBool::new(false));
  let sink = Sink::<u32, _>::on_complete({
    let sink_completed = sink_completed.clone();
    move |_| sink_completed.store(true, Ordering::SeqCst)
  });
  let source = Source::<u32, _>::empty().watch_termination_mat(KeepRight);
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);
  let graph = Source::single(1_u32).via_mat(flow, KeepRight).into_mat(
    Sink::<u32, _>::fold(Vec::<u32>::new(), |mut values, value| {
      values.push(value);
      values
    }),
    KeepBoth,
  );

  let mut materializer = ActorMaterializer::new(
    build_system(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let materialized = graph.run(&mut materializer).expect("run");

  let (right_completion, collected) = materialized.materialized();
  wait_until_ready(right_completion);
  wait_until_ready(collected);
  wait_until_true(sink_completed.as_ref());

  assert_eq!(right_completion.value(), Completion::Ready(Ok(())));
  assert_eq!(collected.value(), Completion::Ready(Ok(Vec::new())));
}

#[test]
fn coupled_termination_flow_cancels_embedded_source_when_wrapped_sink_cancels() {
  let source = Source::<u32, _>::never().watch_termination_mat(KeepRight);
  let sink = Sink::<u32, _>::cancelled();
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);
  let graph = Source::single(1_u32).via_mat(flow, KeepRight).into_mat(
    Sink::<u32, _>::fold(Vec::<u32>::new(), |mut values, value| {
      values.push(value);
      values
    }),
    KeepBoth,
  );

  let mut materializer = ActorMaterializer::new(
    build_system(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let materialized = graph.run(&mut materializer).expect("run");

  let (right_completion, collected) = materialized.materialized();
  wait_until_ready(right_completion);
  wait_until_ready(collected);

  assert_eq!(right_completion.value(), Completion::Ready(Ok(())));
  assert_eq!(collected.value(), Completion::Ready(Ok(Vec::new())));
}

#[test]
fn coupled_termination_flow_from_sink_and_source_is_equivalent_to_flow_from_sink_and_source_coupled() {
  let sink_a = Sink::<u32, _>::ignore();
  let source_a = Source::single(123_u32);
  let sink_b = Sink::<u32, _>::ignore();
  let source_b = Source::single(123_u32);

  let via_factory: Flow<u32, u32, StreamNotUsed> = CoupledTerminationFlow::from_sink_and_source(sink_a, source_a);
  let via_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_sink_and_source_coupled(sink_b, source_b);

  let values_factory = Source::single(0_u32).via(via_factory).run_with_collect_sink().expect("run_with_collect_sink");
  let values_flow = Source::single(0_u32).via(via_flow).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values_factory, values_flow);
  assert_eq!(values_factory, vec![123_u32]);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_is_equivalent_to_flow_from_sink_and_source_coupled_mat() {
  let sink_a = Sink::<u32, _>::ignore().map_materialized_value(|_| 3_u32);
  let source_a = Source::single(99_u32).map_materialized_value(|_| 4_u32);
  let sink_b = Sink::<u32, _>::ignore().map_materialized_value(|_| 3_u32);
  let source_b = Source::single(99_u32).map_materialized_value(|_| 4_u32);

  let via_factory = CoupledTerminationFlow::from_sink_and_source_mat(sink_a, source_a, KeepLeft);
  let via_flow = Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_coupled_mat(sink_b, source_b, KeepLeft);
  let graph_factory =
    Source::single(0_u32).via_mat(via_factory, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);
  let graph_flow = Source::single(0_u32).via_mat(via_flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  assert_eq!(graph_factory.materialized(), &3_u32);
  assert_eq!(graph_flow.materialized(), &3_u32);
}
