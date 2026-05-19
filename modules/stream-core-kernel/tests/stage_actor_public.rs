use core::time::Duration;
use std::time::Instant;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView, Kill, PoisonPill},
    props::Props,
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_kernel_rs::{
  StreamError,
  dsl::{Flow, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight, StreamFuture, StreamNotUsed},
  shape::{Inlet, Outlet, StreamShape},
  stage::{GraphStage, GraphStageLogic, StageActorEnvelope, StageActorReceive, StageContext},
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

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

struct StageActorRecordingLogic {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
  actor:  Option<ActorRef>,
}

impl StageActorRecordingLogic {
  fn new(values: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { values, actor: None }
  }
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for StageActorRecordingLogic {
  fn on_start(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let stage_actor =
      ctx.get_stage_actor(Box::new(RecordingReceive { values: self.values.clone() })).expect("stage actor");
    let same_stage_actor = ctx.stage_actor().expect("initialized stage actor");
    assert_eq!(same_stage_actor.actor_ref().pid(), stage_actor.actor_ref().pid());
    self.actor = Some(stage_actor.actor_ref().clone());
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    let actor = self.actor.as_mut().expect("stage actor initialized");
    actor.try_tell(AnyMessage::new(value)).expect("stage actor delivery");
    ctx.push(value);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct StageActorRecordingStage {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl GraphStage<u32, u32, StreamNotUsed> for StageActorRecordingStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> {
    Box::new(StageActorRecordingLogic::new(self.values.clone()))
  }
}

struct StageActorControlMessageLogic {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
  actor:  Option<ActorRef>,
}

impl StageActorControlMessageLogic {
  fn new(values: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { values, actor: None }
  }
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for StageActorControlMessageLogic {
  fn on_start(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let stage_actor =
      ctx.get_stage_actor(Box::new(RecordingReceive { values: self.values.clone() })).expect("stage actor");
    self.actor = Some(stage_actor.actor_ref().clone());
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    let actor = self.actor.as_mut().expect("stage actor initialized");
    actor.try_tell(AnyMessage::new(PoisonPill)).expect("poison pill ignored");
    actor.try_tell(AnyMessage::new(Kill)).expect("kill ignored");
    actor.try_tell(AnyMessage::new(value)).expect("normal delivery");
    ctx.push(value);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct StageActorControlMessageStage {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl GraphStage<u32, u32, StreamNotUsed> for StageActorControlMessageStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> {
    Box::new(StageActorControlMessageLogic::new(self.values.clone()))
  }
}

#[test]
fn graph_stage_logic_can_receive_messages_through_stage_actor_ref() {
  // Given: StageActor を on_start で初期化する GraphStage
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(StageActorRecordingStage { values: received.clone() });
  let graph = Source::from_array([10_u32, 20]).via(flow).into_mat(
    Sink::fold(Vec::<u32>::new(), |mut values, value| {
      values.push(value);
      values
    }),
    KeepRight,
  );
  let mut materializer = ActorMaterializer::new(
    build_system(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("materializer start");

  // When: ActorMaterializer 経由で stream を実行する
  let materialized = graph.run(&mut materializer).expect("materialize");
  let output = poll_completion(materialized.materialized()).expect("completion");

  // Then: StageActor 経由の message は stage machinery の外で失われない
  assert_eq!(output, vec![10_u32, 20]);
  assert_eq!(*received.lock(), vec![10_u32, 20]);
}

#[test]
fn stage_actor_ref_ignores_poison_pill_and_kill_but_keeps_normal_messages() {
  // Given: control message の後に通常 message を送る GraphStage
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let flow =
    Flow::<u32, u32, StreamNotUsed>::from_graph_stage(StageActorControlMessageStage { values: received.clone() });
  let graph = Source::single(99_u32).via(flow).into_mat(Sink::head(), KeepRight);
  let mut materializer = ActorMaterializer::new(
    build_system(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("materializer start");

  // When: PoisonPill / Kill / normal message を StageActorRef に送る
  let materialized = graph.run(&mut materializer).expect("materialize");
  let output = poll_completion(materialized.materialized()).expect("completion");

  // Then: Pekko と同じく PoisonPill / Kill は StageActor を止めず、通常 message だけ届く
  assert_eq!(output, 99_u32);
  assert_eq!(*received.lock(), vec![99_u32]);
}
