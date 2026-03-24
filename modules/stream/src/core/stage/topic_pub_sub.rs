//! Topic-based pub/sub stream integration.

#[cfg(test)]
mod tests;

use fraktor_actor_rs::core::{
  error::ActorError,
  system::ActorSystem,
  typed::{Behavior, Behaviors, Topic, TopicCommand, TypedProps, actor::TypedActorRef},
};

use super::{ActorSink, ActorSource, Sink, Source, StreamCompletion, StreamDone};
use crate::core::{OverflowStrategy, StreamNotUsed};

/// Topic-based pub/sub stream integration (Pekko `PubSub` equivalent).
///
/// Provides factory methods to create sources and sinks that integrate
/// with the typed [`Topic`] pub/sub actor.
pub struct TopicPubSub;

impl TopicPubSub {
  /// Creates a source that subscribes to a topic and receives published messages.
  ///
  /// Internally spawns a bridge actor that subscribes to the topic
  /// and forwards received messages to the stream via a bounded queue.
  /// The bridge actor is spawned under the system guardian.
  ///
  /// The overflow strategy controls what happens when the internal buffer
  /// is full and the bridge actor receives more messages from the topic.
  #[must_use]
  pub fn source<T>(
    mut topic_actor: TypedActorRef<TopicCommand<T>>,
    buffer_size: usize,
    overflow_strategy: OverflowStrategy,
    system: &ActorSystem,
  ) -> Source<T, StreamNotUsed>
  where
    T: Clone + Send + Sync + 'static, {
    let source = ActorSource::actor_ref::<T>(buffer_size, overflow_strategy);
    let extended = system.extended();

    source.map_materialized_value(move |actor_source_ref| {
      let bridge_props = TypedProps::<T>::from_behavior_factory(move || bridge_behavior(actor_source_ref.clone()));

      // TODO(#1344): bridge アクターの寿命を stream に結び付ける。
      // 現在は stream 終了時に bridge の停止・購読解除が行われない。
      // 短命な source() を繰り返し materialize すると subscriber が残り続ける。
      let child =
        extended.spawn_system_actor(bridge_props.to_untyped()).expect("TopicPubSub: bridge actor の spawn に失敗");
      let bridge_ref = TypedActorRef::<T>::from_untyped(child.actor_ref().clone());
      topic_actor.tell(Topic::subscribe(bridge_ref)).expect("TopicPubSub: topic への subscribe に失敗");

      StreamNotUsed
    })
  }

  /// Creates a sink that publishes each stream element to a topic.
  ///
  /// Each element flowing through the sink is wrapped in a
  /// [`Topic::publish`] command and sent to the topic actor.
  #[must_use]
  pub fn sink<T>(topic_actor: TypedActorRef<TopicCommand<T>>) -> Sink<T, StreamCompletion<StreamDone>>
  where
    T: Clone + Send + Sync + 'static, {
    let mut topic = topic_actor;
    ActorSink::actor_ref(move |msg: T| {
      topic.tell(Topic::publish(msg)).expect("TopicPubSub: topic への publish に失敗");
    })
  }
}

/// Creates the bridge actor behavior that forwards messages to the stream queue.
fn bridge_behavior<T>(actor_source_ref: crate::core::ActorSourceRef<T>) -> Behavior<T>
where
  T: Clone + Send + Sync + 'static, {
  Behaviors::receive_message(move |_ctx, msg: &T| match actor_source_ref.tell(msg.clone()) {
    | crate::core::QueueOfferResult::Enqueued | crate::core::QueueOfferResult::Dropped => Ok(Behaviors::same()),
    | crate::core::QueueOfferResult::QueueClosed => Err(ActorError::recoverable("TopicPubSub: stream queue is closed")),
    | crate::core::QueueOfferResult::Failure(err) => {
      Err(ActorError::recoverable(alloc::format!("TopicPubSub: queue offer failed: {:?}", err)))
    },
  })
}
