//! Placement coordinator core logic.

use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};

use super::{
  ActivationEntry, ActivationError, ActivationRecord, PlacementCommand, PlacementCommandResult,
  PlacementCoordinatorError, PlacementCoordinatorOutcome, PlacementCoordinatorState, PlacementDecision, PlacementEvent,
  PlacementLease, PlacementLocality, PlacementRequestId, PlacementResolution, PlacementSnapshot,
};
use crate::core::{
  grain::{GrainKey, VirtualActorEvent, VirtualActorRegistry},
  identity::{LookupError, PidCacheEvent, RendezvousHasher},
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
struct PendingRequest {
  key:        GrainKey,
  authority:  String,
  decided_at: u64,
  lease:      Option<PlacementLease>,
  activation: Option<ActivationRecord>,
  locality:   PlacementLocality,
}

/// Core placement coordinator (no_std).
pub struct PlacementCoordinatorCore {
  state:                  PlacementCoordinatorState,
  registry:               VirtualActorRegistry,
  authorities:            Vec<String>,
  local_authority:        Option<String>,
  distributed_activation: bool,
  pending:                BTreeMap<PlacementRequestId, PendingRequest>,
  next_request_id:        u64,
  events:                 Vec<PlacementEvent>,
}

impl PlacementCoordinatorCore {
  /// Creates a new placement coordinator.
  #[must_use]
  pub const fn new(cache_capacity: usize, pid_ttl_secs: u64) -> Self {
    Self {
      state:                  PlacementCoordinatorState::Stopped,
      registry:               VirtualActorRegistry::new(cache_capacity, pid_ttl_secs),
      authorities:            Vec::new(),
      local_authority:        None,
      distributed_activation: false,
      pending:                BTreeMap::new(),
      next_request_id:        0,
      events:                 Vec::new(),
    }
  }

  /// Returns current state.
  #[must_use]
  pub const fn state(&self) -> PlacementCoordinatorState {
    self.state
  }

  /// Returns current authority list.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn authorities(&self) -> &[String] {
    &self.authorities
  }

  /// Sets the local authority identifier.
  pub fn set_local_authority(&mut self, authority: impl Into<String>) {
    self.local_authority = Some(authority.into());
  }

  /// Enables or disables distributed activation commands.
  pub const fn set_distributed_activation(&mut self, enabled: bool) {
    self.distributed_activation = enabled;
  }

  /// Starts in member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if the coordinator cannot transition.
  pub const fn start_member(&mut self) -> Result<(), PlacementCoordinatorError> {
    self.state = PlacementCoordinatorState::Member;
    Ok(())
  }

  /// Starts in client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if the coordinator cannot transition.
  pub const fn start_client(&mut self) -> Result<(), PlacementCoordinatorError> {
    self.state = PlacementCoordinatorState::Client;
    Ok(())
  }

  /// Stops the coordinator and clears pending state.
  ///
  /// # Errors
  ///
  /// Returns an error if the coordinator cannot stop.
  pub fn stop(&mut self) -> Result<(), PlacementCoordinatorError> {
    self.state = PlacementCoordinatorState::Stopped;
    self.pending.clear();
    Ok(())
  }

  /// Updates topology authorities.
  pub fn update_topology(&mut self, authorities: Vec<String>) {
    self.registry.invalidate_absent_authorities(&authorities);
    let events = self.collect_registry_events(0);
    self.events.extend(events);
    self.authorities = authorities;
  }

  /// Invalidates activations for a departed authority.
  pub fn invalidate_authority(&mut self, authority: &str) {
    self.registry.invalidate_authority(authority);
    let events = self.collect_registry_events(0);
    self.events.extend(events);
  }

  /// Removes a PID from registry and cache.
  pub fn remove_pid(&mut self, key: &GrainKey) {
    self.registry.remove_activation(key);
    let events = self.collect_registry_events(0);
    self.events.extend(events);
  }

  /// Passivates idle activations.
  pub fn passivate_idle(&mut self, now: u64, idle_ttl_secs: u64) {
    self.registry.passivate_idle(now, idle_ttl_secs);
    let events = self.collect_registry_events(now);
    self.events.extend(events);
  }

  /// Drains PID cache events.
  pub fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    self.registry.drain_cache_events()
  }

  /// Drains placement events.
  pub fn drain_events(&mut self) -> Vec<PlacementEvent> {
    core::mem::take(&mut self.events)
  }

  /// Returns snapshot of current coordinator state.
  #[must_use]
  pub fn snapshot(&self) -> PlacementSnapshot {
    PlacementSnapshot {
      state:           self.state,
      authorities:     self.authorities.clone(),
      local_authority: self.local_authority.clone(),
    }
  }

  /// Resolves placement for a grain key.
  ///
  /// # Errors
  ///
  /// Returns an error when resolution fails or is not ready.
  pub fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementCoordinatorOutcome, LookupError> {
    if matches!(self.state, PlacementCoordinatorState::Stopped | PlacementCoordinatorState::NotReady) {
      return Err(LookupError::NotReady);
    }

    let Some(owner) = RendezvousHasher::select(&self.authorities, key).cloned() else {
      return Err(LookupError::NoAuthority);
    };

    let decision = PlacementDecision { key: key.clone(), authority: owner.clone(), observed_at: now };
    let mut events = Vec::new();
    events.push(PlacementEvent::Resolved { key: key.clone(), authority: owner.clone(), observed_at: now });

    if self.is_remote(&owner) {
      let pid = format!("{}::{}", owner, key.value());
      let resolution = PlacementResolution { decision, locality: PlacementLocality::Remote, pid };
      self.events.extend(events);
      return Ok(PlacementCoordinatorOutcome {
        resolution: Some(resolution),
        commands:   Vec::new(),
        events:     Vec::new(),
      });
    }

    if let Some(pid) = self.registry.cached_pid(key, now) {
      let resolution = PlacementResolution { decision, locality: PlacementLocality::Local, pid };
      events.push(PlacementEvent::Activated {
        key:         key.clone(),
        pid:         resolution.pid.clone(),
        observed_at: now,
      });
      self.events.extend(events);
      return Ok(PlacementCoordinatorOutcome {
        resolution: Some(resolution),
        commands:   Vec::new(),
        events:     Vec::new(),
      });
    }

    if !self.distributed_activation {
      let mut outcome = self.resolve_with_registry(key, now, decision, events)?;
      outcome.events.extend(self.collect_registry_events(now));
      self.events.extend(outcome.events);
      outcome.events = Vec::new();
      return Ok(outcome);
    }

    let request_id = self.next_request_id();
    let pending = PendingRequest {
      key:        key.clone(),
      authority:  owner.clone(),
      decided_at: now,
      lease:      None,
      activation: None,
      locality:   PlacementLocality::Local,
    };
    self.pending.insert(request_id, pending);

    let command = PlacementCommand::TryAcquire { request_id, key: key.clone(), owner, now };
    self.events.extend(events);
    Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events: Vec::new() })
  }

  /// Applies a command result and returns next outcome.
  ///
  /// # Errors
  ///
  /// Returns an error when the request is not recognized.
  pub fn handle_command_result(
    &mut self,
    result: PlacementCommandResult,
  ) -> Result<PlacementCoordinatorOutcome, PlacementCoordinatorError> {
    let request_id = Self::result_request_id(&result);
    let mut events = Vec::new();

    let mut pending =
      self.pending.remove(&request_id).ok_or(PlacementCoordinatorError::UnknownRequest { request_id })?;

    match result {
      | PlacementCommandResult::LockAcquired { result, .. } => match result {
        | Ok(lease) => {
          pending.lease = Some(lease);
          let command = PlacementCommand::LoadActivation { request_id, key: pending.key.clone() };
          self.pending.insert(request_id, pending);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events })
        },
        | Err(err) => {
          let reason = format!("{err:?}");
          events.push(PlacementEvent::LockDenied { key: pending.key.clone(), reason, observed_at: pending.decided_at });
          events.extend(self.collect_registry_events(pending.decided_at));
          self.events.extend(events);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: Vec::new(), events: Vec::new() })
        },
      },
      | PlacementCommandResult::ActivationLoaded { result, .. } => match result {
        | Ok(Some(entry)) => {
          pending.activation = Some(entry.record);
          let command = Self::release_command(request_id, &pending)?;
          self.pending.insert(request_id, pending);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events })
        },
        | Ok(None) => {
          let command = PlacementCommand::EnsureActivation {
            request_id,
            key: pending.key.clone(),
            owner: pending.authority.clone(),
          };
          self.pending.insert(request_id, pending);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events })
        },
        | Err(_) => {
          let command = Self::release_command(request_id, &pending)?;
          self.pending.insert(request_id, pending);
          self.events.extend(events);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events: Vec::new() })
        },
      },
      | PlacementCommandResult::ActivationEnsured { result, .. } => match result {
        | Ok(record) => {
          pending.activation = Some(record.clone());
          let entry = ActivationEntry { owner: pending.authority.clone(), record, observed_at: pending.decided_at };
          let command = PlacementCommand::StoreActivation { request_id, key: pending.key.clone(), entry };
          self.pending.insert(request_id, pending);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events })
        },
        | Err(_) => {
          let command = Self::release_command(request_id, &pending)?;
          self.pending.insert(request_id, pending);
          self.events.extend(events);
          Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events: Vec::new() })
        },
      },
      | PlacementCommandResult::ActivationStored { .. } => {
        let command = Self::release_command(request_id, &pending)?;
        self.pending.insert(request_id, pending);
        self.events.extend(events);
        Ok(PlacementCoordinatorOutcome { resolution: None, commands: vec![command], events: Vec::new() })
      },
      | PlacementCommandResult::LockReleased { result, .. } => {
        let release_result = result;
        let activation = pending.activation.as_ref();

        if release_result.is_err() {
          self.events.extend(events);
          return Ok(PlacementCoordinatorOutcome { resolution: None, commands: Vec::new(), events: Vec::new() });
        }

        if let Some(record) = activation {
          let resolution = self.finalize_resolution(&pending, record);
          events.extend(self.collect_registry_events(pending.decided_at));
          self.events.extend(events);
          return Ok(PlacementCoordinatorOutcome {
            resolution: Some(resolution),
            commands:   Vec::new(),
            events:     Vec::new(),
          });
        }

        self.events.extend(events);
        Ok(PlacementCoordinatorOutcome { resolution: None, commands: Vec::new(), events: Vec::new() })
      },
    }
  }

  fn resolve_with_registry(
    &mut self,
    key: &GrainKey,
    now: u64,
    decision: PlacementDecision,
    events: Vec<PlacementEvent>,
  ) -> Result<PlacementCoordinatorOutcome, LookupError> {
    match self.registry.ensure_activation(key, &self.authorities, now, false, None) {
      | Ok(pid) => {
        let resolution = PlacementResolution { decision, locality: PlacementLocality::Local, pid };
        Ok(PlacementCoordinatorOutcome { resolution: Some(resolution), commands: Vec::new(), events })
      },
      | Err(ActivationError::NoAuthority) => Err(LookupError::NoAuthority),
      | Err(ActivationError::SnapshotMissing { key }) => Err(LookupError::ActivationFailed { key }),
    }
  }

  fn is_remote(&self, owner: &str) -> bool {
    match &self.local_authority {
      | Some(local) => local != owner,
      | None => false,
    }
  }

  const fn next_request_id(&mut self) -> PlacementRequestId {
    self.next_request_id = self.next_request_id.saturating_add(1);
    PlacementRequestId(self.next_request_id)
  }

  fn finalize_resolution(&mut self, pending: &PendingRequest, record: &ActivationRecord) -> PlacementResolution {
    let pid = record.pid.clone();
    self.registry.record_activation(&pending.key, &pending.authority, record, pending.decided_at);
    PlacementResolution {
      decision: PlacementDecision {
        key:         pending.key.clone(),
        authority:   pending.authority.clone(),
        observed_at: pending.decided_at,
      },
      locality: pending.locality,
      pid,
    }
  }

  fn release_command(
    request_id: PlacementRequestId,
    pending: &PendingRequest,
  ) -> Result<PlacementCommand, PlacementCoordinatorError> {
    let Some(lease) = pending.lease.clone() else {
      return Err(PlacementCoordinatorError::UnknownRequest { request_id });
    };
    Ok(PlacementCommand::Release { request_id, lease })
  }

  const fn result_request_id(result: &PlacementCommandResult) -> PlacementRequestId {
    match result {
      | PlacementCommandResult::LockAcquired { request_id, .. } => *request_id,
      | PlacementCommandResult::ActivationLoaded { request_id, .. } => *request_id,
      | PlacementCommandResult::ActivationEnsured { request_id, .. } => *request_id,
      | PlacementCommandResult::ActivationStored { request_id, .. } => *request_id,
      | PlacementCommandResult::LockReleased { request_id, .. } => *request_id,
    }
  }

  fn collect_registry_events(&mut self, now: u64) -> Vec<PlacementEvent> {
    let mut events = Vec::new();
    for event in self.registry.drain_events() {
      match event {
        | VirtualActorEvent::Activated { key, pid, .. }
        | VirtualActorEvent::Hit { key, pid }
        | VirtualActorEvent::Reactivated { key, pid, .. } => {
          events.push(PlacementEvent::Activated { key, pid, observed_at: now });
        },
        | VirtualActorEvent::Passivated { key } => {
          events.push(PlacementEvent::Passivated { key, observed_at: now });
        },
        | VirtualActorEvent::SnapshotMissing { .. } => {},
      }
    }
    events
  }
}
