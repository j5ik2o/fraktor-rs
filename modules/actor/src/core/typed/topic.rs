//! Typed pub/sub topic actor built on top of the receptionist.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  error::ActorError,
  event::logging::LogLevel,
  typed::{
    Behaviors, Listing, Receptionist, ServiceKey, TopicCommand, TopicStats, actor::TypedActorRef, behavior::Behavior,
  },
};

struct TopicState<M>
where
  M: Clone + Send + Sync + 'static, {
  topic_instances:   Vec<TypedActorRef<TopicCommand<M>>>,
  local_subscribers: Vec<TypedActorRef<M>>,
}

/// Factory and helpers for the typed pub/sub topic actor.
pub struct Topic;

impl Topic {
  /// Creates a topic actor behavior for the provided topic name.
  ///
  /// Publish uses one of two exclusive delivery paths. When
  /// `topic_instances` is empty, the topic behaves as a local-only pub/sub and
  /// forwards messages directly to `local_subscribers`. When
  /// `topic_instances` is non-empty, the topic forwards a
  /// `MessagePublished` command to each registered topic instance, and each
  /// instance then delivers to its own `local_subscribers`. Being present in
  /// `topic_instances` does not cause duplicate local delivery because only one
  /// branch executes for each publish.
  ///
  /// # Errors
  ///
  /// Returns a fatal actor error when the actor system does not provide a
  /// receptionist or when the topic cannot install its receptionist adapter.
  #[must_use]
  pub fn behavior<M>(topic_name: impl Into<String>) -> Behavior<TopicCommand<M>>
  where
    M: Clone + Send + Sync + 'static, {
    let topic_name = topic_name.into();
    let topic_key = ServiceKey::<TopicCommand<M>>::new(topic_name);
    let state =
      ArcShared::new(RuntimeMutex::new(TopicState { topic_instances: Vec::new(), local_subscribers: Vec::new() }));

    Behaviors::setup(move |ctx| {
      let Some(receptionist) = ctx.system().receptionist_ref() else {
        ctx.system().emit_log(LogLevel::Error, "topic requires receptionist", Some(ctx.pid()));
        return Behaviors::stopped();
      };
      let adapter =
        match ctx.message_adapter(move |listing: Listing| Ok(TopicCommand::topic_instances_updated(listing))) {
          | Ok(adapter) => adapter,
          | Err(error) => {
            let message = alloc::format!("topic failed to create receptionist adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
            return Behaviors::stopped();
          },
        };
      let mut receptionist_ref = receptionist.clone();
      if let Err(error) = receptionist_ref.try_tell(Receptionist::subscribe(&topic_key, adapter)) {
        let message = alloc::format!("topic failed to subscribe to receptionist: {:?}", error);
        ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
        return Behaviors::stopped();
      }

      let state_for_message = state.clone();
      let topic_key_for_messages = topic_key.clone();
      Behaviors::receive_message(move |ctx, command: &TopicCommand<M>| {
        let mut state = state_for_message.lock();
        match command.clone().into_kind() {
          | super::topic_command::TopicCommandKind::Publish(message) => {
            if state.topic_instances.is_empty() {
              publish_local(&state.local_subscribers, &message);
            } else {
              publish_instances(&state.topic_instances, &message);
            }
          },
          | super::topic_command::TopicCommandKind::Subscribe(subscriber) => {
            if !state.local_subscribers.iter().any(|existing| existing.pid() == subscriber.pid()) {
              ctx
                .watch_with(&subscriber, TopicCommand::subscriber_terminated(subscriber.pid()))
                .map_err(|error| ActorError::from_send_error(&error))?;
              state.local_subscribers.push(subscriber);
              if state.local_subscribers.len() == 1 {
                let mut receptionist = receptionist.clone();
                receptionist
                  .try_tell(Receptionist::register(&topic_key_for_messages, ctx.self_ref()))
                  .map_err(|error| ActorError::from_send_error(&error))?;
              }
            }
          },
          | super::topic_command::TopicCommandKind::Unsubscribe(subscriber) => {
            if let Err(e) = ctx.unwatch(&subscriber) {
              ctx.system().emit_log(
                LogLevel::Warn,
                alloc::format!("topic failed to unwatch subscriber: {:?}", e),
                Some(ctx.pid()),
              );
            }
            remove_subscriber(&mut state.local_subscribers, subscriber.pid());
            deregister_if_empty(&state, &mut receptionist.clone(), &topic_key_for_messages, ctx);
          },
          | super::topic_command::TopicCommandKind::GetTopicStats { reply_to } => {
            let mut reply_to = reply_to;
            reply_to
              .try_tell(TopicStats::new(state.local_subscribers.len(), state.topic_instances.len()))
              .map_err(|error| ActorError::from_send_error(&error))?;
          },
          | super::topic_command::TopicCommandKind::TopicInstancesUpdated(listing) => {
            state.topic_instances = listing.typed_refs::<TopicCommand<M>>()?;
          },
          | super::topic_command::TopicCommandKind::MessagePublished(message) => {
            publish_local(&state.local_subscribers, &message);
          },
          | super::topic_command::TopicCommandKind::SubscriberTerminated(pid) => {
            remove_subscriber(&mut state.local_subscribers, pid);
            deregister_if_empty(&state, &mut receptionist.clone(), &topic_key_for_messages, ctx);
          },
        }
        Ok(Behaviors::same())
      })
    })
  }

  /// Creates a publish command.
  #[must_use]
  pub const fn publish<M>(message: M) -> TopicCommand<M>
  where
    M: Clone + Send + Sync + 'static, {
    TopicCommand::publish(message)
  }

  /// Creates a subscribe command.
  #[must_use]
  pub const fn subscribe<M>(subscriber: TypedActorRef<M>) -> TopicCommand<M>
  where
    M: Clone + Send + Sync + 'static, {
    TopicCommand::subscribe(subscriber)
  }

  /// Creates an unsubscribe command.
  #[must_use]
  pub const fn unsubscribe<M>(subscriber: TypedActorRef<M>) -> TopicCommand<M>
  where
    M: Clone + Send + Sync + 'static, {
    TopicCommand::unsubscribe(subscriber)
  }

  /// Creates a get-topic-stats command.
  #[must_use]
  pub const fn get_topic_stats<M>(reply_to: TypedActorRef<TopicStats>) -> TopicCommand<M>
  where
    M: Clone + Send + Sync + 'static, {
    TopicCommand::get_topic_stats(reply_to)
  }
}

fn deregister_if_empty<M>(
  state: &TopicState<M>,
  receptionist: &mut TypedActorRef<crate::core::typed::ReceptionistCommand>,
  topic_key: &ServiceKey<TopicCommand<M>>,
  ctx: &mut crate::core::typed::actor::TypedActorContext<'_, TopicCommand<M>>,
) where
  M: Clone + Send + Sync + 'static, {
  if state.local_subscribers.is_empty()
    && let Err(error) = receptionist.try_tell(Receptionist::deregister(topic_key, ctx.self_ref()))
  {
    ctx.system().emit_log(
      LogLevel::Warn,
      alloc::format!("topic failed to deregister from receptionist: {:?}", error),
      Some(ctx.pid()),
    );
  }
}

fn remove_subscriber<M>(subscribers: &mut Vec<TypedActorRef<M>>, pid: crate::core::actor::Pid)
where
  M: Clone + Send + Sync + 'static, {
  subscribers.retain(|subscriber| subscriber.pid() != pid);
}

fn publish_local<M>(subscribers: &[TypedActorRef<M>], message: &M)
where
  M: Clone + Send + Sync + 'static, {
  for subscriber in subscribers {
    let mut subscriber = subscriber.clone();
    if let Err(_error) = subscriber.try_tell(message.clone()) {}
  }
}

fn publish_instances<M>(topic_instances: &[TypedActorRef<TopicCommand<M>>], message: &M)
where
  M: Clone + Send + Sync + 'static, {
  for topic in topic_instances {
    let mut topic = topic.clone();
    if let Err(_error) = topic.try_tell(TopicCommand::message_published(message.clone())) {}
  }
}
