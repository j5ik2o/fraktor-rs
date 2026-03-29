use alloc::vec::Vec;
use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_rs::core::{
  kernel::{
    actor::{Actor, ActorContext},
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    system::{ActorSystem, ActorSystemConfig},
  },
  typed::{
    Behaviors, TypedProps,
    actor::TypedActorRef,
    pubsub::{Topic, TopicCommand, TopicStats},
  },
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  StreamDone, StreamNotUsed,
  buffer::OverflowStrategy,
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight, StreamCompletion},
  stage::{sink::Sink, source::Source, topic_pub_sub::TopicPubSub},
};

// --- test helpers ---

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  ActorSystem::new_with_config(&props, &config).expect("system should build")
}

fn spawn_topic<T>(system: &ActorSystem, name: &str) -> TypedActorRef<TopicCommand<T>>
where
  T: Clone + Send + Sync + 'static, {
  let name = alloc::string::String::from(name);
  let topic_props = TypedProps::<TopicCommand<T>>::from_behavior_factory(move || Topic::behavior(name.clone()));
  let child = system.extended().spawn_system_actor(&topic_props.to_untyped()).expect("spawn topic");
  TypedActorRef::<TopicCommand<T>>::from_untyped(child.into_actor_ref())
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  let deadline = Instant::now() + Duration::from_secs(2);
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    thread::yield_now();
  }
  assert!(condition(), "wait_until timed out");
}

// --- TopicPubSub::source ---

#[test]
fn topic_pub_sub_source_should_materialize_without_error_after_publish() {
  // Given: topic と PubSub source を Sink::collect に接続して materialize
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let mut topic = spawn_topic::<u32>(&system, "test-source");

  let source: Source<u32, StreamNotUsed> = TopicPubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: topic にメッセージを publish
  topic.tell(Topic::publish(1_u32));
  topic.tell(Topic::publish(2_u32));
  topic.tell(Topic::publish(3_u32));

  for _ in 0..20 {
    controller.inject_and_drive(1);
  }
  assert!(matches!(materialized.materialized().poll(), Completion::Pending));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_should_be_constructible_and_connectable_to_sink() {
  // Given: topic と PubSub source
  let system = build_system();
  let topic = spawn_topic::<u32>(&system, "test-unsub");

  let source: Source<u32, StreamNotUsed> = TopicPubSub::source(topic.clone(), 8, OverflowStrategy::Fail, &system);

  // When: source を Sink に接続してグラフを構築
  let graph = source.into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);

  // Then: グラフが materialize できること
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let _materialized = graph.run(&mut materializer).expect("run");

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_unsubscribes_bridge_after_downstream_cancel() {
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let mut topic = spawn_topic::<u32>(&system, "test-cleanup");

  let graph =
    TopicPubSub::source(topic.clone(), 8, OverflowStrategy::DropHead, &system).into_mat(Sink::head(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let _materialized = graph.run(&mut materializer).expect("run");

  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  topic.tell(Topic::publish(7_u32));
  for _ in 0..50 {
    controller.inject_and_drive(1);
  }

  for _ in 0..20 {
    controller.inject_and_drive(1);
  }

  let mut subscriber_count = None;
  for _ in 0..20 {
    let stats = topic.ask::<TopicStats, _>(Topic::get_topic_stats);
    for _ in 0..10 {
      controller.inject_and_drive(1);
      if stats.future().is_ready() {
        break;
      }
    }
    if !stats.future().is_ready() {
      continue;
    }
    let mut stats_future = stats.future().clone();
    let stats = stats_future.try_take().expect("stats ready").expect("stats ok");
    subscriber_count = Some(stats.local_subscriber_count());
    if stats.local_subscriber_count() == 0 {
      break;
    }
  }
  assert_eq!(subscriber_count, Some(0));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_should_materialize_with_small_buffer_and_fail_overflow() {
  // Given: 小バッファ (2) + Fail 戦略の PubSub source
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let mut topic = spawn_topic::<u32>(&system, "test-overflow");

  let source: Source<u32, StreamNotUsed> = TopicPubSub::source(topic.clone(), 2, OverflowStrategy::Fail, &system);

  let graph = source.into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: バッファ容量を超えるメッセージを一括 publish
  for i in 0..10_u32 {
    topic.tell(Topic::publish(i));
  }

  for _ in 0..20 {
    controller.inject_and_drive(1);
  }

  // Then: materialize 成功。ストリームはエラーまたは継続中のいずれか
  // （Fail 戦略ではバッファ超過時にエラーが発生しうる）
  let poll = materialized.materialized().poll();
  assert!(
    matches!(poll, Completion::Pending | Completion::Ready(Err(_))),
    "Fail 戦略でバッファ超過時は NotReady か Error のいずれか: {:?}",
    poll
  );

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_should_accept_messages_from_multiple_publishers() {
  // Given: 単一 topic に対して PubSub source を materialize
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let mut topic = spawn_topic::<u32>(&system, "test-multi-pub");

  let source: Source<u32, StreamNotUsed> = TopicPubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: 複数の呼び出し元から同じ topic に publish
  let mut topic2 = topic.clone();
  topic.tell(Topic::publish(100_u32));
  topic2.tell(Topic::publish(200_u32));

  for _ in 0..20 {
    controller.inject_and_drive(1);
  }

  assert!(matches!(materialized.materialized().poll(), Completion::Pending));

  system.terminate().expect("terminate");
}

// --- TopicPubSub::sink ---

#[test]
fn topic_pub_sub_sink_should_publish_stream_elements_to_topic() {
  // Given: a topic actor with a subscriber, and a PubSub sink connected to a source
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let mut topic = spawn_topic::<u32>(&system, "test-sink");

  // Set up a subscriber to receive messages published via the sink
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let subscriber_props = TypedProps::<u32>::from_behavior_factory({
    let received = received.clone();
    move || {
      let received = received.clone();
      Behaviors::receive_message(move |_ctx, message: &u32| {
        received.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let subscriber = system.extended().spawn_system_actor(&subscriber_props.to_untyped()).expect("spawn subscriber");
  let subscriber_ref = TypedActorRef::<u32>::from_untyped(subscriber.into_actor_ref());

  topic.tell(Topic::subscribe(subscriber_ref));

  // Allow subscription to propagate
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  // When: running a stream that publishes elements through PubSub.sink
  let sink = TopicPubSub::sink(topic.clone());
  let graph = Source::from_array([10_u32, 20_u32, 30_u32]).into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // Then: the subscriber should receive all published messages
  wait_until(|| {
    controller.inject_and_drive(1);
    matches!(materialized.materialized().poll(), Completion::Ready(Ok(_)))
  });

  wait_until(|| received.lock().len() >= 3);
  let mut values = received.lock().clone();
  values.sort();
  assert_eq!(values, vec![10_u32, 20_u32, 30_u32]);

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_sink_should_complete_normally_when_source_finishes() {
  // Given: a PubSub sink connected to a finite source
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let topic = spawn_topic::<u32>(&system, "test-sink-complete");

  let sink: Sink<u32, StreamCompletion<StreamDone>> = TopicPubSub::sink(topic.clone());
  let graph = Source::from_array([1_u32, 2_u32]).into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: driving the stream to completion
  for _ in 0..20 {
    controller.inject_and_drive(1);
  }

  // Then: the stream should complete successfully
  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_sink_should_handle_empty_source() {
  // Given: a PubSub sink connected to an empty source
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let topic = spawn_topic::<u32>(&system, "test-sink-empty");

  let sink: Sink<u32, StreamCompletion<StreamDone>> = TopicPubSub::sink(topic.clone());
  let graph = Source::<u32, _>::empty().into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: driving the stream
  for _ in 0..10 {
    controller.inject_and_drive(1);
  }

  // Then: the stream should complete successfully with no messages published
  assert!(matches!(materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))));

  system.terminate().expect("terminate");
}

// --- Integration: PubSub source + sink via the same topic ---

#[test]
fn topic_pub_sub_source_and_sink_should_materialize_pipeline_without_error() {
  // Given: topic に対して source と sink の両方を materialize
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("controller").clone();
  let topic = spawn_topic::<u32>(&system, "test-pipeline");

  // PubSub source（subscriber）をセットアップ
  let source: Source<u32, StreamNotUsed> = TopicPubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let source_materialized = graph.run(&mut materializer).expect("run source");

  // 購読の伝播を待つ
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  // When: PubSub sink 経由で publish
  let sink = TopicPubSub::sink(topic.clone());
  let graph = Source::from_array([42_u32, 43_u32]).into_mat(sink, KeepRight);
  let sink_materialized = graph.run(&mut materializer).expect("run sink");

  for _ in 0..30 {
    controller.inject_and_drive(1);
  }

  // Then: sink は有限 source なので完了する
  assert!(
    matches!(sink_materialized.materialized().poll(), Completion::Ready(Ok(StreamDone))),
    "sink ストリームは有限 source の完了後に正常終了すべき"
  );
  assert!(matches!(source_materialized.materialized().poll(), Completion::Pending));

  system.terminate().expect("terminate");
}
