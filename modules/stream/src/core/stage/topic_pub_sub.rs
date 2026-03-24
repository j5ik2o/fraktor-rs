//! Topic-based pub/sub stream integration.

#[cfg(test)]
mod tests;

use fraktor_actor_rs::core::{
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

      // TODO: bridge アクターの寿命を stream に結び付ける。
      // 現在は stream 終了時に bridge の停止・購読解除が行われない。
      // 短命な source() を繰り返し materialize すると subscriber が残り続ける。
      if let Ok(child) = extended.spawn_system_actor(bridge_props.to_untyped()) {
        let bridge_ref = TypedActorRef::<T>::from_untyped(child.actor_ref().clone());
        // Best-effort: 購読失敗時はメッセージが受信されないが、stream 自体は有効。
        // 安全な理由: topic actor が停止済みの場合のみ失敗し、その場合 stream は空で完了する。
        let _result = topic_actor.tell(Topic::subscribe(bridge_ref));
      }

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
      // Best-effort: topic actor が停止している場合、メッセージは静かに落ちる。
      // 安全な理由: sink の契約上、publish は fire-and-forget であり、
      // topic actor 停止時のデータロスは許容される設計。
      let _result = topic.tell(Topic::publish(msg));
    })
  }
}

/// Creates the bridge actor behavior that forwards messages to the stream queue.
fn bridge_behavior<T>(actor_source_ref: crate::core::ActorSourceRef<T>) -> Behavior<T>
where
  T: Clone + Send + Sync + 'static, {
  Behaviors::receive_message(move |_ctx, msg: &T| {
    // Forward topic message to the stream queue.
    // The queue applies the configured overflow strategy internally;
    // QueueOfferResult is intentionally not inspected because the overflow
    // policy has already been applied by the bounded queue.
    let _result = actor_source_ref.tell(msg.clone());
    Ok(Behaviors::same())
  })
}
