//! Stream public authoring API showcase.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example stream_authoring_apis`

use std::{thread, time::Duration};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  StreamError,
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source, StreamRefs},
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepLeft, KeepRight, StreamNotUsed},
  shape::{Inlet, Outlet, StreamShape},
  stage::{
    GraphStage, GraphStageLogic, StageActorEnvelope, StageActorReceive, StageContext, SubSinkInlet,
    SubSinkInletHandler, SubSourceOutlet, SubSourceOutletHandler,
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn poll_until_ready<T: Clone>(
  completion: &fraktor_stream_core_rs::core::materialization::StreamCompletion<T>,
  max_ticks: usize,
) -> Option<Result<T, StreamError>> {
  for _ in 0..max_ticks {
    if let Completion::Ready(result) = completion.poll() {
      return Some(result);
    }
    thread::sleep(Duration::from_millis(1));
  }
  None
}

struct RecordingReceive {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl StageActorReceive for RecordingReceive {
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(value) = envelope.message().downcast_ref::<u32>() {
      self.values.lock().push(*value);
    }
    Ok(())
  }
}

struct RecordingStageLogic {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
  actor:  Option<ActorRef>,
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for RecordingStageLogic {
  fn on_start(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let stage_actor =
      ctx.get_stage_actor(Box::new(RecordingReceive { values: self.values.clone() })).expect("stage actor");
    self.actor = Some(stage_actor.actor_ref().clone());
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    self.actor.as_mut().expect("stage actor initialized").try_tell(AnyMessage::new(value)).expect("stage actor tell");
    ctx.push(value);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct RecordingStage {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl GraphStage<u32, u32, StreamNotUsed> for RecordingStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> {
    Box::new(RecordingStageLogic { values: self.values.clone(), actor: None })
  }
}

struct ExampleSubSinkHandler;

impl SubSinkInletHandler<u32> for ExampleSubSinkHandler {
  fn on_push(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

struct ExampleSubSourceHandler;

impl SubSourceOutletHandler<u32> for ExampleSubSourceHandler {
  fn on_pull(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

#[allow(clippy::print_stdout)]
fn main() {
  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");

  let graph_dsl_flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1)).expect("add flow");
  });
  let graph_dsl_graph = Source::from_array([1_u32, 2, 3]).via(graph_dsl_flow).into_mat(Sink::collect(), KeepRight);
  let graph_dsl_materialized = graph_dsl_graph.run(&mut materializer).expect("graph dsl run");
  let graph_dsl_values =
    poll_until_ready(graph_dsl_materialized.materialized(), 64).expect("graph dsl completion").expect("graph dsl");
  println!("graph dsl values: {graph_dsl_values:?}");

  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let stage_flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(RecordingStage { values: received.clone() });
  let stage_graph = Source::from_array([10_u32, 20]).via(stage_flow).into_mat(
    Sink::fold(Vec::new(), |mut acc, value| {
      acc.push(value);
      acc
    }),
    KeepRight,
  );
  let materialized = stage_graph.run(&mut materializer).expect("stage graph run");
  let stage_values =
    poll_until_ready(materialized.materialized(), 64).expect("stage graph completion").expect("stage graph");
  println!("stage graph values: {stage_values:?}, actor received: {:?}", *received.lock());

  let stream_ref_graph = Source::from_array([7_u32, 8, 9]).into_mat(StreamRefs::source_ref::<u32>(), KeepRight);
  let source_ref = stream_ref_graph.run(&mut materializer).expect("source ref graph run").into_materialized();
  let remote_graph = source_ref.into_source().into_mat(
    Sink::fold(Vec::new(), |mut acc, value| {
      acc.push(value);
      acc
    }),
    KeepRight,
  );
  let remote_materialized = remote_graph.run(&mut materializer).expect("remote source run");
  let stream_ref_values =
    poll_until_ready(remote_materialized.materialized(), 64).expect("stream ref completion").expect("stream ref");
  println!("stream ref values: {stream_ref_values:?}");

  let mut sub_sink = SubSinkInlet::<u32>::new("example-sub-sink");
  sub_sink.set_handler(ExampleSubSinkHandler);
  let sub_sink_graph = Source::single(1_u32).into_mat(sub_sink.sink(), KeepRight);
  println!("sub sink materialized: {:?}", sub_sink_graph.materialized());

  let mut sub_source = SubSourceOutlet::<u32>::new("example-sub-source");
  sub_source.set_handler(ExampleSubSourceHandler);
  let sub_source_graph = sub_source.source().into_mat(Sink::<u32, _>::ignore(), KeepLeft);
  println!("sub source materialized: {:?}", sub_source_graph.materialized());

  materializer.shutdown().expect("materializer shutdown");
}
