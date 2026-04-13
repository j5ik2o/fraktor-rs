//! Cluster public API built on top of the cluster extension.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeSet,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_path::ActorPathParser,
    actor_ref::ActorRef,
    messaging::{AnyMessage, AskError, AskResponse, AskResult},
    scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  },
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
    subscriber_handle,
  },
  system::ActorSystem,
  util::futures::ActorFutureShared,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::{
  ClusterApiError, ClusterError, ClusterEvent, ClusterEventType, ClusterExtension, ClusterRequestError,
  ClusterResolveError, ClusterSubscriptionInitialStateMode,
  grain::{GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainMetricsShared},
  identity::{ClusterIdentity, LookupError},
  placement::PlacementEvent,
};

const CLUSTER_EVENT_STREAM_NAME: &str = "cluster";

struct ClusterEventFilterSubscriber {
  subscriber:  EventStreamSubscriberShared,
  event_types: BTreeSet<ClusterEventType>,
}

impl ClusterEventFilterSubscriber {
  fn new(subscriber: EventStreamSubscriberShared, event_types: BTreeSet<ClusterEventType>) -> Self {
    Self { subscriber, event_types }
  }

  fn matches_event(event: &EventStreamEvent, event_types: &BTreeSet<ClusterEventType>) -> bool {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      return event_types.iter().any(|event_type| event_type.matches(cluster_event));
    }
    false
  }
}

impl EventStreamSubscriber for ClusterEventFilterSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if Self::matches_event(event, &self.event_types) {
      self.subscriber.notify(event);
    }
  }
}

/// Cluster API facade bound to an actor system.
pub struct ClusterApi {
  system:    ActorSystem,
  extension: ArcShared<ClusterExtension>,
}

impl ClusterApi {
  /// Retrieves the cluster API from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn try_from_system(system: &ActorSystem) -> Result<Self, ClusterApiError> {
    let extension =
      system.extended().extension_by_type::<ClusterExtension>().ok_or(ClusterApiError::ExtensionNotInstalled)?;
    Ok(Self { system: system.clone(), extension })
  }

  pub(crate) const fn system(&self) -> &ActorSystem {
    &self.system
  }

  pub(crate) fn grain_metrics_shared(&self) -> Option<GrainMetricsShared> {
    self.extension.grain_metrics_shared()
  }

  /// Resolves an identity into an actor reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster is not started, the kind is not registered,
  /// PID lookup fails, or actor resolution fails.
  pub fn get(&self, identity: &ClusterIdentity) -> Result<ActorRef, ClusterResolveError> {
    self.resolve_actor_ref(identity)
  }

  /// Sends a request and returns the ask response handle.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution fails, sending fails, or timeout scheduling fails.
  pub fn request(
    &self,
    identity: &ClusterIdentity,
    message: AnyMessage,
    timeout: Option<Duration>,
  ) -> Result<AskResponse, ClusterRequestError> {
    let mut actor_ref = self.get(identity).map_err(ClusterRequestError::ResolveFailed)?;
    let response = actor_ref.ask(message);
    if let Some(timeout) = timeout {
      self.schedule_timeout(timeout, response.future().clone())?;
    }
    Ok(response)
  }

  /// Sends a request and returns the shared response future.
  ///
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution fails, sending fails, or timeout scheduling fails.
  pub fn request_future(
    &self,
    identity: &ClusterIdentity,
    message: AnyMessage,
    timeout: Option<Duration>,
  ) -> Result<ActorFutureShared<AskResult>, ClusterRequestError> {
    let response = self.request(identity, message, timeout)?;
    let (_, future) = response.into_parts();
    Ok(future)
  }

  /// Explicitly downs the provided member authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or downing fails.
  pub fn down(&self, authority: &str) -> Result<(), ClusterError> {
    self.extension.down(authority)
  }

  /// Requests a member join for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or join processing fails.
  pub fn join(&self, authority: &str) -> Result<(), ClusterError> {
    self.extension.join(authority)
  }

  /// Requests a graceful member leave for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or leave processing fails.
  pub fn leave(&self, authority: &str) -> Result<(), ClusterError> {
    self.extension.leave(authority)
  }

  /// Subscribes to cluster events with explicit initial-state mode and event filters.
  ///
  /// `ClusterSubscriptionInitialStateMode::AsSnapshot` always delivers one
  /// `ClusterEvent::CurrentClusterState` as the first message.
  ///
  /// Panics when `event_types` is empty.
  ///
  /// # Panics
  ///
  /// Panics when `event_types` is empty.
  #[must_use]
  pub fn subscribe(
    &self,
    subscriber: &EventStreamSubscriberShared,
    initial_state_mode: ClusterSubscriptionInitialStateMode,
    event_types: &[ClusterEventType],
  ) -> EventStreamSubscription {
    assert!(!event_types.is_empty(), "at least one cluster event type is required");

    let event_type_set = to_event_type_set(event_types);
    let filtered = subscriber_handle(ClusterEventFilterSubscriber::new(subscriber.clone(), event_type_set));
    let event_stream = self.system.event_stream();

    match initial_state_mode {
      | ClusterSubscriptionInitialStateMode::AsEvents => {
        let (subscription_id, snapshot) = event_stream.with_write(|stream| stream.subscribe(filtered.clone()));
        for event in &snapshot {
          filtered.notify(event);
        }
        EventStreamSubscription::new(event_stream, subscription_id)
      },
      | ClusterSubscriptionInitialStateMode::AsSnapshot => {
        // Subscribe first to avoid event gap between snapshot and registration.
        let subscription_id = event_stream.with_write(|stream| stream.subscribe_no_replay(filtered));
        let initial_event = {
          let core = self.extension.core_shared();
          let (state, observed_at) = core.with_lock(|core| core.current_cluster_state_snapshot());
          ClusterEvent::CurrentClusterState { state, observed_at }
        };
        let payload = AnyMessage::new(initial_event);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        subscriber.notify(&extension_event);
        EventStreamSubscription::new(event_stream, subscription_id)
      },
    }
  }

  /// Subscribes to cluster events without replaying buffered events.
  ///
  /// Panics when `event_types` is empty.
  ///
  /// # Panics
  ///
  /// Panics when `event_types` is empty.
  #[must_use]
  pub fn subscribe_no_replay(
    &self,
    subscriber: &EventStreamSubscriberShared,
    event_types: &[ClusterEventType],
  ) -> EventStreamSubscription {
    assert!(!event_types.is_empty(), "at least one cluster event type is required");

    let filtered =
      subscriber_handle(ClusterEventFilterSubscriber::new(subscriber.clone(), to_event_type_set(event_types)));
    let event_stream = self.system.event_stream();
    let subscription_id = event_stream.with_write(|stream| stream.subscribe_no_replay(filtered));
    EventStreamSubscription::new(event_stream, subscription_id)
  }

  /// Unsubscribes from event stream notifications by subscription identifier.
  pub fn unsubscribe(&self, subscription_id: u64) {
    self.system.event_stream().unsubscribe(subscription_id);
  }

  fn resolve_actor_ref(&self, identity: &ClusterIdentity) -> Result<ActorRef, ClusterResolveError> {
    let key = identity.key();
    let now = self.current_time_secs();
    let (pid_result, placement_events) = {
      let core = self.extension.core_shared();
      core.with_lock(|guard| {
        if guard.mode().is_none() {
          return Err(ClusterResolveError::ClusterNotStarted);
        }
        if !guard.is_kind_registered(identity.kind()) {
          return Err(ClusterResolveError::KindNotRegistered { kind: identity.kind().to_string() });
        }
        let resolution = guard.resolve_pid(&key, now).map_err(|error| match error {
          | LookupError::Pending => ClusterResolveError::LookupPending,
          | _ => ClusterResolveError::LookupFailed,
        });
        let events = guard.drain_placement_events();
        Ok((resolution.map(|value| value.pid), events))
      })?
    };
    self.publish_activation_events(placement_events);
    let pid = pid_result?;

    let (authority, path) = split_pid(&pid)?;
    let system_name = self.system.state().system_name();
    let canonical = format!("fraktor.tcp://{system_name}@{authority}/{path}");
    let actor_path = ActorPathParser::parse(&canonical)
      .map_err(|error| ClusterResolveError::InvalidPidFormat { pid: pid.clone(), reason: error.to_string() })?;

    self.system.resolve_actor_ref(actor_path).map_err(ClusterResolveError::ActorRefResolve)
  }

  fn current_time_secs(&self) -> u64 {
    self.system.state().monotonic_now().as_secs()
  }

  fn schedule_timeout(
    &self,
    timeout: Duration,
    future: ActorFutureShared<AskResult>,
  ) -> Result<(), ClusterRequestError> {
    let runnable = ArcShared::new(TimeoutRunnable { future });

    let command = SchedulerCommand::RunRunnable { runnable };
    let result = self.system.state().scheduler().with_write(|scheduler| scheduler.schedule_once(timeout, command));
    result.map(|_| ()).map_err(|error| ClusterRequestError::TimeoutScheduleFailed { reason: format!("{error:?}") })
  }

  fn publish_activation_events(&self, events: Vec<PlacementEvent>) {
    let metrics = self.grain_metrics_shared();
    if metrics.is_none() && events.is_empty() {
      return;
    }
    let event_stream = self.system.event_stream();
    for event in events {
      match event {
        | PlacementEvent::Activated { key, pid, .. } => {
          publish_grain_event(&event_stream, GrainEvent::ActivationCreated { key, pid });
          if let Some(metrics) = &metrics {
            metrics.with_write(|inner| inner.record_activation_created());
          }
        },
        | PlacementEvent::Passivated { key, .. } => {
          publish_grain_event(&event_stream, GrainEvent::ActivationPassivated { key });
          if let Some(metrics) = &metrics {
            metrics.with_write(|inner| inner.record_activation_passivated());
          }
        },
        | _ => {},
      }
    }
  }
}

fn split_pid(pid: &str) -> Result<(&str, &str), ClusterResolveError> {
  let (authority, path) = pid.split_once("::").ok_or_else(|| ClusterResolveError::InvalidPidFormat {
    pid:    pid.to_string(),
    reason: "missing :: delimiter".into(),
  })?;
  if authority.is_empty() {
    return Err(ClusterResolveError::InvalidPidFormat { pid: pid.to_string(), reason: "authority is empty".into() });
  }
  if path.is_empty() {
    return Err(ClusterResolveError::InvalidPidFormat { pid: pid.to_string(), reason: "path is empty".into() });
  }
  Ok((authority, path))
}

fn publish_grain_event(event_stream: &EventStreamShared, event: GrainEvent) {
  let payload = AnyMessage::new(event);
  let extension_event = EventStreamEvent::Extension { name: String::from(GRAIN_EVENT_STREAM_NAME), payload };
  event_stream.publish(&extension_event);
}

struct TimeoutRunnable {
  future: ActorFutureShared<AskResult>,
}

impl SchedulerRunnable for TimeoutRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    let waker =
      self.future.with_write(|inner| if inner.is_ready() { None } else { inner.complete(Err(AskError::Timeout)) });
    if let Some(waker) = waker {
      waker.wake();
    }
  }
}

fn to_event_type_set(event_types: &[ClusterEventType]) -> BTreeSet<ClusterEventType> {
  event_types.iter().copied().collect()
}
