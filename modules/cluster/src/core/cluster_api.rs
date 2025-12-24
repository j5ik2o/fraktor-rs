//! Cluster public API built on top of the cluster extension.

#[cfg(test)]
mod tests;

use alloc::{format, string::ToString};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{actor_path::ActorPathParser, actor_ref::ActorRefGeneric},
  dispatch::scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  messaging::{AnyMessageGeneric, AskResponseGeneric},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ClusterApiError, ClusterExtensionGeneric, ClusterIdentity, ClusterRequestError, ClusterResolveError,
};

/// Cluster API facade bound to an actor system.
pub struct ClusterApiGeneric<TB: RuntimeToolbox + 'static> {
  system:    ActorSystemGeneric<TB>,
  extension: ArcShared<ClusterExtensionGeneric<TB>>,
}

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
  /// # Errors
  ///
  /// Returns an error if resolution fails, sending fails, or timeout scheduling fails.
  pub fn request_future(
    &self,
    identity: &ClusterIdentity,
    message: AnyMessageGeneric<TB>,
    timeout: Option<Duration>,
  ) -> Result<fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>, ClusterRequestError>
  {
    let response = self.request(identity, message, timeout)?;
    let (_, future) = response.into_parts();
    Ok(future)
  }

  fn resolve_actor_ref(&self, identity: &ClusterIdentity) -> Result<ActorRefGeneric<TB>, ClusterResolveError> {
    let key = identity.key();
    let now = self.current_time_secs();
    let pid = {
      let core = self.extension.core_shared();
      let mut guard = core.lock();
      if guard.mode().is_none() {
        return Err(ClusterResolveError::ClusterNotStarted);
      }
      if !guard.is_kind_registered(identity.kind()) {
        return Err(ClusterResolveError::KindNotRegistered { kind: identity.kind().to_string() });
      }
      let resolution = guard.resolve_pid(&key, now).map_err(|_| ClusterResolveError::LookupFailed)?;
      resolution.pid
    };

    let (authority, path) = split_pid(&pid)?;
    let canonical = format!("fraktor.tcp://cellactor@{authority}/{path}");
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
    future: fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>,
  ) -> Result<(), ClusterRequestError> {
    let runnable = ArcShared::new(TimeoutRunnable { future });

    let command = SchedulerCommand::RunRunnable { runnable, dispatcher: None };
    let result = self.system.state().scheduler().with_write(|scheduler| scheduler.schedule_once(timeout, command));
    result.map(|_| ()).map_err(|error| ClusterRequestError::TimeoutScheduleFailed { reason: format!("{error:?}") })
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

struct TimeoutRunnable<TB: RuntimeToolbox + 'static> {
  future: fraktor_actor_rs::core::futures::ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerRunnable for TimeoutRunnable<TB> {
  fn run(&self, _batch: &ExecutionBatch) {
    let waker = self.future.with_write(|inner| {
      if inner.is_ready() {
        None
      } else {
        let message = AnyMessageGeneric::new(ClusterRequestError::Timeout);
        inner.complete(message)
      }
    });
    if let Some(waker) = waker {
      waker.wake();
    }
  }
}
