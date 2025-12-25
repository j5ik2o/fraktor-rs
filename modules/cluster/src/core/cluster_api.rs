//! Cluster public API built on top of the cluster extension.

#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{actor_path::ActorPathParser, actor_ref::ActorRefGeneric},
  event::stream::EventStreamEvent,
  messaging::{AnyMessageGeneric, AskError, AskResponseGeneric, AskResult},
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ClusterApiError, ClusterExtensionGeneric, ClusterIdentity, ClusterRequestError, ClusterResolveError,
  GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainMetricsSharedGeneric, PlacementEvent,
};

/// Cluster API facade bound to an actor system.
pub struct ClusterApiGeneric<TB: RuntimeToolbox + 'static> {
  system:    ActorSystemGeneric<TB>,
  extension: ArcShared<ClusterExtensionGeneric<TB>>,
}

/// Cluster API bound to the default no_std toolbox.
pub type ClusterApi = ClusterApiGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ClusterApiGeneric<TB> {
  /// Retrieves the cluster API from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn try_from_system(system: &ActorSystemGeneric<TB>) -> Result<Self, ClusterApiError> {
    let extension = system
      .extended()
      .extension_by_type::<ClusterExtensionGeneric<TB>>()
      .ok_or(ClusterApiError::ExtensionNotInstalled)?;
    Ok(Self { system: system.clone(), extension })
  }

  pub(crate) const fn system(&self) -> &ActorSystemGeneric<TB> {
    &self.system
  }

  pub(crate) fn grain_metrics_shared(&self) -> Option<GrainMetricsSharedGeneric<TB>> {
    self.extension.grain_metrics_shared()
  }

  /// Resolves an identity into an actor reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster is not started, the kind is not registered,
  /// PID lookup fails, or actor resolution fails.
  pub fn get(&self, identity: &ClusterIdentity) -> Result<ActorRefGeneric<TB>, ClusterResolveError> {
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
    message: AnyMessageGeneric<TB>,
    timeout: Option<Duration>,
  ) -> Result<AskResponseGeneric<TB>, ClusterRequestError> {
    let actor_ref = self.get(identity).map_err(ClusterRequestError::ResolveFailed)?;
    let response =
      actor_ref.ask(message).map_err(|error| ClusterRequestError::SendFailed { reason: format!("{error:?}") })?;

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
    message: AnyMessageGeneric<TB>,
    timeout: Option<Duration>,
  ) -> Result<fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AskResult<TB>, TB>, ClusterRequestError> {
    let response = self.request(identity, message, timeout)?;
    let (_, future) = response.into_parts();
    Ok(future)
  }

  fn resolve_actor_ref(&self, identity: &ClusterIdentity) -> Result<ActorRefGeneric<TB>, ClusterResolveError> {
    let key = identity.key();
    let now = self.current_time_secs();
    let (pid_result, placement_events) = {
      let core = self.extension.core_shared();
      let mut guard = core.lock();
      if guard.mode().is_none() {
        return Err(ClusterResolveError::ClusterNotStarted);
      }
      if !guard.is_kind_registered(identity.kind()) {
        return Err(ClusterResolveError::KindNotRegistered { kind: identity.kind().to_string() });
      }
      let resolution = guard.resolve_pid(&key, now).map_err(|error| match error {
        | crate::core::LookupError::Pending => ClusterResolveError::LookupPending,
        | _ => ClusterResolveError::LookupFailed,
      });
      let events = guard.drain_placement_events();
      (resolution.map(|value| value.pid), events)
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
    future: fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AskResult<TB>, TB>,
  ) -> Result<(), ClusterRequestError> {
    let runnable = ArcShared::new(TimeoutRunnable { future });

    let command = SchedulerCommand::RunRunnable { runnable, dispatcher: None };
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

fn publish_grain_event<TB: RuntimeToolbox + 'static>(
  event_stream: &fraktor_actor_rs::core::event::stream::EventStreamSharedGeneric<TB>,
  event: GrainEvent,
) {
  let payload = AnyMessageGeneric::new(event);
  let extension_event = EventStreamEvent::Extension { name: String::from(GRAIN_EVENT_STREAM_NAME), payload };
  event_stream.publish(&extension_event);
}

struct TimeoutRunnable<TB: RuntimeToolbox + 'static> {
  future: fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AskResult<TB>, TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerRunnable for TimeoutRunnable<TB> {
  fn run(&self, _batch: &ExecutionBatch) {
    let waker =
      self.future.with_write(|inner| if inner.is_ready() { None } else { inner.complete(Err(AskError::Timeout)) });
    if let Some(waker) = waker {
      waker.wake();
    }
  }
}
