//! Topic-based pub/sub stream integration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_actor_core_rs::core::{
  kernel::{
    actor::{
      ChildRef,
      error::{ActorError, SendError},
    },
    system::ActorSystem,
  },
  typed::{
    Behavior, TypedActorRef, TypedProps,
    dsl::Behaviors,
    pubsub::{Topic, TopicCommand},
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  StageContext, StreamDone, StreamFuture, actor_sink::ActorSink, actor_source::ActorSource, flow::Flow, sink::Sink,
  source::Source,
};
use crate::core::{
  OverflowStrategy, QueueOfferResult, StreamError,
  r#impl::queue::ActorSourceRef,
  materialization::{KeepLeft, StreamNotUsed},
  shape::{Inlet, Outlet, StreamShape},
  stage::{GraphStage, GraphStageLogic},
};

/// Topic-based pub/sub stream integration (Pekko `PubSub` equivalent).
///
/// Provides factory methods to create sources and sinks that integrate
/// with the typed [`Topic`] pub/sub actor.
pub struct TopicPubSub;

struct TopicSourceCleanupState<T>
where
  T: Clone + Send + Sync + 'static, {
  topic_actor:  TypedActorRef<TopicCommand<T>>,
  bridge_ref:   TypedActorRef<T>,
  bridge_child: ChildRef,
}

#[derive(Clone)]
struct TopicSourceCleanup<T>
where
  T: Clone + Send + Sync + 'static, {
  state: ArcShared<SpinSyncMutex<Option<TopicSourceCleanupState<T>>>>,
}

impl<T> TopicSourceCleanup<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn new() -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(None)) }
  }

  fn install(&self, topic_actor: TypedActorRef<TopicCommand<T>>, bridge_ref: TypedActorRef<T>, bridge_child: ChildRef) {
    let mut guard = self.state.lock();
    *guard = Some(TopicSourceCleanupState { topic_actor, bridge_ref, bridge_child });
  }

  fn cleanup(&self) {
    {
      let mut guard = self.state.lock();
      let Some(state) = guard.take() else {
        return;
      };
      // stream 終了後の best-effort cleanup であり、topic actor が既に停止していても整合性は壊れない。
      if let Err(_error) = send_topic_command(&state.topic_actor, Topic::unsubscribe(state.bridge_ref.clone())) {
        // best-effort cleanup のため、unsubscribe 失敗は無視してよい。
      }
      // bridge がすでに終了している場合、追加 stop は不要。
      if let Err(_error) = state.bridge_child.stop() {
        // stream 終了後の best-effort cleanup であり、bridge が既に止まっていても整合性は壊れない。
      }
    }
  }
}

struct TopicSourceCleanupStage<T>
where
  T: Clone + Send + Sync + 'static, {
  cleanup: TopicSourceCleanup<T>,
}

impl<T> GraphStage<T, T, StreamNotUsed> for TopicSourceCleanupStage<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn shape(&self) -> StreamShape<T, T> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<T, T, StreamNotUsed> + Send> {
    Box::new(TopicSourceCleanupLogic { cleanup: self.cleanup.clone() })
  }
}

struct TopicSourceCleanupLogic<T>
where
  T: Clone + Send + Sync + 'static, {
  cleanup: TopicSourceCleanup<T>,
}

impl<T> GraphStageLogic<T, T, StreamNotUsed> for TopicSourceCleanupLogic<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<T, T>) {
    let value = ctx.grab();
    ctx.push(value);
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<T, T>, _error: StreamError) {
    self.cleanup.cleanup();
  }

  fn on_stop(&mut self, _ctx: &mut dyn StageContext<T, T>) {
    self.cleanup.cleanup();
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed
  }
}

impl TopicPubSub {
  /// Creates a source that subscribes to a topic and receives published messages.
  ///
  /// Internally spawns a bridge actor that subscribes to the topic
  /// and forwards received messages to the stream via a bounded queue.
  /// The bridge actor is spawned under the system guardian.
  ///
  /// The overflow strategy controls what happens when the internal buffer
  /// is full and the bridge actor receives more messages from the topic.
  ///
  /// # Panics
  ///
  /// Panics when spawning the bridge actor or subscribing it to the topic fails.
  #[must_use]
  pub fn source<T>(
    topic_actor: TypedActorRef<TopicCommand<T>>,
    buffer_size: usize,
    overflow_strategy: OverflowStrategy,
    system: &ActorSystem,
  ) -> Source<T, StreamNotUsed>
  where
    T: Clone + Send + Sync + 'static, {
    let cleanup = TopicSourceCleanup::new();
    let source_ref = ActorSource::actor_ref::<T>(buffer_size, overflow_strategy);
    let source =
      source_ref.via_mat(Flow::from_graph_stage(TopicSourceCleanupStage { cleanup: cleanup.clone() }), KeepLeft);
    let extended = system.extended();

    source.map_materialized_value(move |actor_source_ref: ActorSourceRef<T>| {
      let bridge_props = TypedProps::<T>::from_behavior_factory(move || bridge_behavior(actor_source_ref.clone()));
      #[allow(clippy::expect_used)]
      let child =
        extended.spawn_system_actor(bridge_props.to_untyped()).expect("TopicPubSub: bridge actor の spawn に失敗");
      let bridge_ref = TypedActorRef::<T>::from_untyped(child.clone().into_actor_ref());
      if let Err(error) = send_topic_command(&topic_actor, Topic::subscribe(bridge_ref.clone())) {
        panic!("TopicPubSub: topic actor への subscribe に失敗: {error:?}");
      }
      cleanup.install(topic_actor.clone(), bridge_ref, child);

      StreamNotUsed
    })
  }

  /// Creates a sink that publishes each stream element to a topic.
  ///
  /// Each element flowing through the sink is wrapped in a
  /// [`Topic::publish`] command and sent to the topic actor.
  #[must_use]
  pub fn sink<T>(topic_actor: TypedActorRef<TopicCommand<T>>) -> Sink<T, StreamFuture<StreamDone>>
  where
    T: Clone + Send + Sync + 'static, {
    let topic = topic_actor;
    ActorSink::actor_ref_with_result(move |msg: T| {
      send_topic_command(&topic, Topic::publish(msg)).map_err(|error| StreamError::from_send_error(&error))
    })
  }
}

fn send_topic_command<T>(
  topic_actor: &TypedActorRef<TopicCommand<T>>,
  command: TopicCommand<T>,
) -> Result<(), SendError>
where
  T: Clone + Send + Sync + 'static, {
  let mut topic_actor = topic_actor.clone();
  topic_actor.try_tell(command)
}

/// Creates the bridge actor behavior that forwards messages to the stream queue.
fn bridge_behavior<T>(actor_source_ref: ActorSourceRef<T>) -> Behavior<T>
where
  T: Clone + Send + Sync + 'static, {
  let actor_source_ref = ArcShared::new(SpinSyncMutex::new(actor_source_ref));
  Behaviors::receive_message(move |_ctx, msg: &T| match actor_source_ref.lock().tell(msg.clone()) {
    | QueueOfferResult::Enqueued | QueueOfferResult::Dropped => Ok(Behaviors::same()),
    | QueueOfferResult::QueueClosed => Err(ActorError::recoverable("TopicPubSub: stream queue is closed")),
    | QueueOfferResult::Failure(err) => {
      Err(ActorError::recoverable(alloc::format!("TopicPubSub: queue offer failed: {:?}", err)))
    },
  })
}
