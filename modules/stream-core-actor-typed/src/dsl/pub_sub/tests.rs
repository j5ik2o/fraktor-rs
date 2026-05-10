use alloc::{string::String, vec::Vec};
use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRef, NullSender},
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_actor_core_typed_rs::{
  TypedActorRef, TypedActorSystem, TypedProps,
  dsl::Behaviors,
  pubsub::{Topic, TopicCommand, TopicStats},
};
use fraktor_stream_core_kernel_rs::{
  BoundedSourceQueue, OverflowStrategy, QueueOfferResult,
  dsl::{Sink, Source},
  r#impl::queue::ActorSourceRef,
  materialization::{
    ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight, StreamDone, StreamFuture, StreamNotUsed,
  },
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{bridge_behavior, queue_offer_result_to_behavior, subscribe_bridge};
use crate::dsl::PubSub;

// --- test helpers ---

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  TypedActorSystem::<u32>::create_from_props(&guardian_props, config).expect("system should build").into_untyped()
}

fn spawn_topic<T>(system: &ActorSystem, name: &str) -> TypedActorRef<TopicCommand<T>>
where
  T: Clone + Send + Sync + 'static, {
  let name = String::from(name);
  let topic_props = TypedProps::<TopicCommand<T>>::from_behavior_factory(move || Topic::behavior(name.clone()));
  let child = system.extended().spawn_system_actor(&topic_props.to_untyped()).expect("spawn topic");
  TypedActorRef::<TopicCommand<T>>::from_untyped(child.into_actor_ref())
}

fn null_typed_ref<M>(pid: u64, name: &str) -> TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  let actor_ref = ActorRef::with_canonical_path(Pid::new(pid, 0), NullSender, ActorPath::root().child(name));
  TypedActorRef::<M>::from_untyped(actor_ref)
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

// --- PubSub::source ---

#[test]
fn topic_pub_sub_source_should_materialize_without_error_after_publish() {
  // Given: topic と PubSub source を Sink::collect に接続して materialize
  let system = build_system();
  let mut topic = spawn_topic::<u32>(&system, "test-source");

  let source: Source<u32, StreamNotUsed> = PubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamFuture<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: topic にメッセージを publish
  topic.tell(Topic::publish(1_u32));
  topic.tell(Topic::publish(2_u32));
  topic.tell(Topic::publish(3_u32));

  // Pending のままであることを確認（有限 source でないので完了しない）
  assert!(matches!(materialized.materialized().value(), Completion::Pending));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_should_be_constructible_and_connectable_to_sink() {
  // Given: topic と PubSub source
  let system = build_system();
  let topic = spawn_topic::<u32>(&system, "test-unsub");

  let source: Source<u32, StreamNotUsed> = PubSub::source(topic.clone(), 8, OverflowStrategy::Fail, &system);

  // When: source を Sink に接続してグラフを構築
  let graph = source.into_mat(Sink::<u32, StreamFuture<Vec<u32>>>::collect(), KeepRight);

  // Then: グラフが materialize できること
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let _materialized = graph.run(&mut materializer).expect("run");

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_unsubscribes_bridge_after_downstream_cancel() {
  let system = build_system();
  let mut topic = spawn_topic::<u32>(&system, "test-cleanup");

  let graph = PubSub::source(topic.clone(), 8, OverflowStrategy::DropHead, &system).into_mat(Sink::head(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let _materialized = graph.run(&mut materializer).expect("run");

  topic.tell(Topic::publish(7_u32));

  let mut subscriber_count = None;
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    let stats = topic.ask::<TopicStats, _>(Topic::get_topic_stats);
    let ask_deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < ask_deadline {
      if stats.future().is_ready() {
        break;
      }
      thread::yield_now();
    }
    if !stats.future().is_ready() {
      thread::yield_now();
      continue;
    }
    let mut stats_future = stats.future().clone();
    let stats = stats_future.try_take().expect("stats ready").expect("stats ok");
    subscriber_count = Some(stats.local_subscriber_count());
    if stats.local_subscriber_count() == 0 {
      break;
    }
    thread::yield_now();
  }
  assert_eq!(subscriber_count, Some(0));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_source_should_materialize_with_small_buffer_and_fail_overflow() {
  // Given: 小バッファ (2) + Fail 戦略の PubSub source
  let system = build_system();
  let mut topic = spawn_topic::<u32>(&system, "test-overflow");

  let source: Source<u32, StreamNotUsed> = PubSub::source(topic.clone(), 2, OverflowStrategy::Fail, &system);

  let graph = source.into_mat(Sink::<u32, StreamFuture<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: バッファ容量を超えるメッセージを一括 publish
  for i in 0..10_u32 {
    topic.tell(Topic::publish(i));
  }

  // バッファ超過の結果を待つ（エラーになるか Pending のまま）
  let deadline = Instant::now() + Duration::from_secs(2);
  while Instant::now() < deadline {
    if matches!(materialized.materialized().value(), Completion::Ready(_)) {
      break;
    }
    thread::yield_now();
  }

  // Then: materialize 成功。ストリームはエラーまたは継続中のいずれか
  // （Fail 戦略ではバッファ超過時にエラーが発生しうる）
  let poll = materialized.materialized().value();
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
  let mut topic = spawn_topic::<u32>(&system, "test-multi-pub");

  let source: Source<u32, StreamNotUsed> = PubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamFuture<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: 複数の呼び出し元から同じ topic に publish
  let mut topic2 = topic.clone();
  topic.tell(Topic::publish(100_u32));
  topic2.tell(Topic::publish(200_u32));

  // Pending のままであることを確認（無限 source なので完了しない）
  assert!(matches!(materialized.materialized().value(), Completion::Pending));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_bridge_should_report_queue_offer_failure() {
  let system = build_system();
  let actor_source_ref = ActorSourceRef::new(BoundedSourceQueue::new(1, OverflowStrategy::Fail));
  let observed_source_ref = actor_source_ref.clone();
  let bridge_props = TypedProps::<u32>::from_behavior_factory(move || bridge_behavior(actor_source_ref.clone()));
  let bridge = system.extended().spawn_system_actor(&bridge_props.to_untyped()).expect("spawn bridge");
  let mut bridge_ref = TypedActorRef::<u32>::from_untyped(bridge.into_actor_ref());

  bridge_ref.tell(1_u32);
  bridge_ref.tell(2_u32);

  wait_until(|| observed_source_ref.is_closed());
  assert!(observed_source_ref.is_closed());

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_bridge_should_report_queue_closed() {
  let result = queue_offer_result_to_behavior::<u32>(QueueOfferResult::QueueClosed);

  assert!(result.is_err());
}

#[test]
fn topic_pub_sub_source_should_panic_when_topic_ref_rejects_subscribe() {
  let topic = null_typed_ref::<TopicCommand<u32>>(9_999, "closed-topic");
  let bridge = null_typed_ref::<u32>(10_000, "bridge");

  let result = catch_unwind(AssertUnwindSafe(|| {
    subscribe_bridge(&topic, bridge);
  }));

  assert!(result.is_err());
}

// --- PubSub::sink ---

#[test]
fn topic_pub_sub_sink_should_publish_stream_elements_to_topic() {
  // Given: a topic actor with a subscriber, and a PubSub sink connected to a source
  let system = build_system();
  let mut topic = spawn_topic::<u32>(&system, "test-sink");

  // Set up a subscriber to receive messages published via the sink
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
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

  // Allow subscription to propagate: poll until subscriber count is 1
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    let stats = topic.ask::<TopicStats, _>(Topic::get_topic_stats);
    let ask_deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < ask_deadline {
      if stats.future().is_ready() {
        break;
      }
      thread::yield_now();
    }
    if stats.future().is_ready() {
      let mut sf = stats.future().clone();
      if let Some(Ok(s)) = sf.try_take() {
        if s.local_subscriber_count() >= 1 {
          break;
        }
      }
    }
    thread::yield_now();
  }

  // When: running a stream that publishes elements through PubSub.sink
  let sink = PubSub::sink(topic.clone());
  let graph = Source::from_array([10_u32, 20_u32, 30_u32]).into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // Then: the subscriber should receive all published messages
  wait_until(|| matches!(materialized.materialized().value(), Completion::Ready(Ok(_))));

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
  let topic = spawn_topic::<u32>(&system, "test-sink-complete");

  let sink: Sink<u32, StreamFuture<StreamDone>> = PubSub::sink(topic.clone());
  let graph = Source::from_array([1_u32, 2_u32]).into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: waiting for the stream to complete
  wait_until(|| matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));

  // Then: the stream should complete successfully
  assert!(matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));

  system.terminate().expect("terminate");
}

#[test]
fn topic_pub_sub_sink_should_handle_empty_source() {
  // Given: a PubSub sink connected to an empty source
  let system = build_system();
  let topic = spawn_topic::<u32>(&system, "test-sink-empty");

  let sink: Sink<u32, StreamFuture<StreamDone>> = PubSub::sink(topic.clone());
  let graph = Source::<u32, _>::empty().into_mat(sink, KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let materialized = graph.run(&mut materializer).expect("run");

  // When: waiting for the stream to complete
  wait_until(|| matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));

  // Then: the stream should complete successfully with no messages published
  assert!(matches!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));

  system.terminate().expect("terminate");
}

// --- Integration: PubSub source + sink via the same topic ---

#[test]
fn topic_pub_sub_source_and_sink_should_materialize_pipeline_without_error() {
  // Given: topic に対して source と sink の両方を materialize
  let system = build_system();
  let mut topic = spawn_topic::<u32>(&system, "test-pipeline");

  // PubSub source（subscriber）をセットアップ
  let source: Source<u32, StreamNotUsed> = PubSub::source(topic.clone(), 16, OverflowStrategy::DropHead, &system);
  let graph = source.into_mat(Sink::<u32, StreamFuture<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());
  materializer.start().expect("start materializer");
  let source_materialized = graph.run(&mut materializer).expect("run source");

  // 購読の伝播を待つ: subscriber_count が 1 になるまでスピン
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    let stats = topic.ask::<TopicStats, _>(Topic::get_topic_stats);
    let ask_deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < ask_deadline {
      if stats.future().is_ready() {
        break;
      }
      thread::yield_now();
    }
    if stats.future().is_ready() {
      let mut sf = stats.future().clone();
      if let Some(Ok(s)) = sf.try_take() {
        if s.local_subscriber_count() >= 1 {
          break;
        }
      }
    }
    thread::yield_now();
  }

  // When: PubSub sink 経由で publish
  let sink = PubSub::sink(topic.clone());
  let graph = Source::from_array([42_u32, 43_u32]).into_mat(sink, KeepRight);
  let sink_materialized = graph.run(&mut materializer).expect("run sink");

  // Then: sink は有限 source なので完了する
  wait_until(|| matches!(sink_materialized.materialized().value(), Completion::Ready(Ok(StreamDone))));

  assert!(
    matches!(sink_materialized.materialized().value(), Completion::Ready(Ok(StreamDone))),
    "sink ストリームは有限 source の完了後に正常終了すべき"
  );
  assert!(matches!(source_materialized.materialized().value(), Completion::Pending));

  system.terminate().expect("terminate");
}
