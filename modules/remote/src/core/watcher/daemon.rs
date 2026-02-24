//! Watches remote actors on behalf of local watchers.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, Pid, actor_path::ActorPathParts, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::sync_mutex_like::SyncMutexLike};

use super::{command::RemoteWatcherCommand, heartbeat::Heartbeat, heartbeat_rsp::HeartbeatRsp};
use crate::core::{
  endpoint_association::QuarantineReason,
  failure_detector::{
    FailureDetector,
    phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
  },
  remoting_extension::{RemotingControl, RemotingControlShared, RemotingError},
};

const HEARTBEAT_UNREACHABLE_REASON: &str = "heartbeat unreachable";

/// System actor that proxies watch/unwatch commands to the remoting control plane.
pub(crate) struct RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:                 RemotingControlShared<TB>,
  watchers:                Vec<(Pid, ActorPathParts)>,
  authority_uids:          BTreeMap<String, u64>,
  failure_detectors:       BTreeMap<String, PhiFailureDetector>,
  failure_detector_config: PhiFailureDetectorConfig,
  #[cfg(any(test, feature = "test-support"))]
  rewatch_count:           usize,
}

impl<TB> RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(control: RemotingControlShared<TB>) -> Self {
    Self {
      control,
      watchers: Vec::new(),
      authority_uids: BTreeMap::new(),
      failure_detectors: BTreeMap::new(),
      failure_detector_config: PhiFailureDetectorConfig::default(),
      #[cfg(any(test, feature = "test-support"))]
      rewatch_count: 0,
    }
  }

  /// Spawns the daemon under the system guardian hierarchy.
  pub(crate) fn spawn(
    system: &ActorSystemGeneric<TB>,
    control: RemotingControlShared<TB>,
  ) -> Result<ActorRefGeneric<TB>, RemotingError> {
    let props = PropsGeneric::from_fn({
      let handle = control.clone();
      move || RemoteWatcherDaemon::new(handle.clone())
    })
    .with_name("remote-watcher-daemon");
    let actor = system.extended().spawn_system_actor(&props).map_err(RemotingError::from)?;
    Ok(actor.actor_ref().clone())
  }

  fn handle_command(&mut self, command: &RemoteWatcherCommand, now_millis: u64) -> Result<(), RemotingError> {
    match command {
      | RemoteWatcherCommand::Watch { target, watcher } => self.handle_watch(target, *watcher, now_millis)?,
      | RemoteWatcherCommand::Unwatch { target, watcher } => self.handle_unwatch(target, *watcher),
      | RemoteWatcherCommand::Heartbeat { heartbeat } => self.handle_heartbeat_probe(heartbeat, now_millis),
      | RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp } => {
        self.handle_heartbeat_response(heartbeat_rsp, now_millis)?
      },
      | RemoteWatcherCommand::ReapUnreachable => self.handle_reap_unreachable(now_millis)?,
      | RemoteWatcherCommand::HeartbeatTick => self.handle_heartbeat_tick()?,
    }
    Ok(())
  }

  fn handle_watch(&mut self, target: &ActorPathParts, watcher: Pid, now_millis: u64) -> Result<(), RemotingError> {
    if !self
      .watchers
      .iter()
      .any(|(existing_watcher, existing_target)| *existing_watcher == watcher && existing_target == target)
    {
      self.watchers.push((watcher, target.clone()));
    }
    self.control.lock().associate(target)?;
    if let Some(authority) = Self::authority_from_parts(target) {
      self.failure_detector_for(&authority).heartbeat(now_millis);
    }
    Ok(())
  }

  fn handle_unwatch(&mut self, target: &ActorPathParts, watcher: Pid) {
    self
      .watchers
      .retain(|(existing_watcher, existing_target)| !(*existing_watcher == watcher && existing_target == target));
    if let Some(authority) = Self::authority_from_parts(target)
      && !self.is_watching_authority(&authority)
    {
      self.authority_uids.remove(&authority);
      self.failure_detectors.remove(&authority);
    }
  }

  fn handle_heartbeat_probe(&mut self, heartbeat: &Heartbeat, now_millis: u64) {
    if self.is_watching_authority(heartbeat.authority()) {
      self.failure_detector_for(heartbeat.authority()).heartbeat(now_millis);
    }
  }

  fn handle_heartbeat_response(&mut self, heartbeat_rsp: &HeartbeatRsp, now_millis: u64) -> Result<(), RemotingError> {
    let authority = heartbeat_rsp.authority();
    if self.is_watching_authority(authority) {
      if self.authority_uids.get(authority).copied() != Some(heartbeat_rsp.uid) {
        self.rewatch_authority(authority)?;
      }
      self.authority_uids.insert(authority.to_string(), heartbeat_rsp.uid);
      self.failure_detector_for(authority).heartbeat(now_millis);
    }
    Ok(())
  }

  fn handle_reap_unreachable(&mut self, now_millis: u64) -> Result<(), RemotingError> {
    for authority in self.watched_authorities() {
      if !self.failure_detector_for(&authority).is_available(now_millis) {
        let reason = QuarantineReason::new(HEARTBEAT_UNREACHABLE_REASON);
        self.control.lock().quarantine(&authority, &reason)?;
      }
    }
    Ok(())
  }

  fn handle_heartbeat_tick(&mut self) -> Result<(), RemotingError> {
    for authority in self.watched_authorities() {
      self.rewatch_authority(&authority)?;
    }
    Ok(())
  }

  fn watched_authorities(&self) -> Vec<String> {
    let mut authorities = Vec::new();
    for (_, target) in &self.watchers {
      if let Some(authority) = Self::authority_from_parts(target)
        && !authorities.iter().any(|existing| existing == &authority)
      {
        authorities.push(authority);
      }
    }
    authorities
  }

  fn rewatch_authority(&mut self, authority: &str) -> Result<(), RemotingError> {
    #[cfg(any(test, feature = "test-support"))]
    {
      self.rewatch_count += 1;
    }
    for (_, target) in &self.watchers {
      if Self::authority_from_parts(target).as_deref() == Some(authority) {
        self.control.lock().associate(target)?;
      }
    }
    Ok(())
  }

  fn authority_from_parts(parts: &ActorPathParts) -> Option<String> {
    parts.authority_endpoint()
  }

  fn is_watching_authority(&self, authority: &str) -> bool {
    self.watchers.iter().any(|(_, target)| Self::authority_from_parts(target).as_deref() == Some(authority))
  }

  fn failure_detector_for(&mut self, authority: &str) -> &mut PhiFailureDetector {
    self
      .failure_detectors
      .entry(authority.to_string())
      .or_insert_with(|| PhiFailureDetector::new(self.failure_detector_config.clone()))
  }
}

impl<TB> Actor<TB> for RemoteWatcherDaemon<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<RemoteWatcherCommand>() {
      let now_millis = ctx.system().state().monotonic_now().as_millis() as u64;
      self.handle_command(command, now_millis).map_err(|error| ActorError::recoverable(error.to_string()))?;
    }
    Ok(())
  }
}
