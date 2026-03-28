//! Commands accepted by the typed topic actor.

#[cfg(test)]
mod tests;

use super::topic_stats::TopicStats;
use crate::core::{
  kernel::actor::Pid,
  typed::{actor::TypedActorRef, receptionist::Listing},
};

/// Commands handled by [`Topic`](crate::core::typed::pubsub::Topic).
///
/// User code constructs commands through [`Topic`](crate::core::typed::pubsub::Topic)
/// factory methods only. Internal coordination messages stay crate-private.
///
/// ```compile_fail
/// use fraktor_actor_rs::core::typed::pubsub::TopicCommand;
///
/// let _ = TopicCommand::MessagePublished(1_u32);
/// ```
#[derive(Clone)]
pub struct TopicCommand<M>(TopicCommandKind<M>)
where
  M: Clone + Send + Sync + 'static;

#[derive(Clone)]
pub(crate) enum TopicCommandKind<M>
where
  M: Clone + Send + Sync + 'static, {
  Publish(M),
  Subscribe(TypedActorRef<M>),
  Unsubscribe(TypedActorRef<M>),
  GetTopicStats { reply_to: TypedActorRef<TopicStats> },
  TopicInstancesUpdated(Listing),
  MessagePublished(M),
  SubscriberTerminated(Pid),
}

impl<M> TopicCommand<M>
where
  M: Clone + Send + Sync + 'static,
{
  pub(crate) const fn publish(message: M) -> Self {
    Self(TopicCommandKind::Publish(message))
  }

  pub(crate) const fn subscribe(subscriber: TypedActorRef<M>) -> Self {
    Self(TopicCommandKind::Subscribe(subscriber))
  }

  pub(crate) const fn unsubscribe(subscriber: TypedActorRef<M>) -> Self {
    Self(TopicCommandKind::Unsubscribe(subscriber))
  }

  pub(crate) const fn get_topic_stats(reply_to: TypedActorRef<TopicStats>) -> Self {
    Self(TopicCommandKind::GetTopicStats { reply_to })
  }

  pub(crate) const fn topic_instances_updated(listing: Listing) -> Self {
    Self(TopicCommandKind::TopicInstancesUpdated(listing))
  }

  pub(crate) const fn message_published(message: M) -> Self {
    Self(TopicCommandKind::MessagePublished(message))
  }

  pub(crate) const fn subscriber_terminated(pid: Pid) -> Self {
    Self(TopicCommandKind::SubscriberTerminated(pid))
  }

  pub(crate) fn into_kind(self) -> TopicCommandKind<M> {
    self.0
  }
}
