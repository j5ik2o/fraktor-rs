//! Pure local state machine for distributed-data protocol commands.

#[cfg(test)]
#[path = "replicator_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};

use super::{
  Delete, DeleteResponse, DeleteWriteOutcome, Get, GetResponse, ReadConsistency, ReplicatedData, ReplicatorEntry,
  ReplicatorSettings, Subscribe, SubscribeResponse, Unsubscribe, Update, UpdateResponse, UpdateWriteOutcome,
  WriteConsistency,
};

/// Result of applying one Replicator command against local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicatorOutcome<D: ReplicatedData, C, S> {
  /// Protocol response for the handled command, when present.
  pub response:      Option<ReplicatorResponse<D, C>>,
  /// Subscriber notifications produced by the mutation.
  pub notifications: Vec<(S, SubscribeResponse<D>)>,
}

impl<D: ReplicatedData, C, S> ReplicatorOutcome<D, C, S> {
  /// Creates an outcome with no response and no notifications.
  #[must_use]
  pub const fn empty() -> Self {
    Self { response: None, notifications: Vec::new() }
  }
}

/// Protocol response produced by the Replicator core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicatorResponse<D: ReplicatedData, C> {
  /// Response to a get command.
  Get(GetResponse<D, C>),
  /// Response to an update command.
  Update(UpdateResponse<D, C>),
  /// Response to a delete command.
  Delete(DeleteResponse<D, C>),
  /// Subscribe completed.
  Subscribe,
  /// Unsubscribe completed.
  Unsubscribe,
}

/// Local Replicator state machine backed by an in-memory entry map.
#[derive(Debug, Clone)]
pub struct ReplicatorCore<D: ReplicatedData, S> {
  settings:    ReplicatorSettings,
  entries:     BTreeMap<String, ReplicatorEntry<D>>,
  subscribers: BTreeMap<String, BTreeSet<S>>,
}

impl<D: ReplicatedData, S: Clone> ReplicatorCore<D, S> {
  /// Creates a new Replicator core with the provided settings.
  #[must_use]
  pub fn new(settings: ReplicatorSettings) -> Self {
    Self { settings, entries: BTreeMap::new(), subscribers: BTreeMap::new() }
  }

  /// Returns the configured settings.
  #[must_use]
  pub const fn settings(&self) -> &ReplicatorSettings {
    &self.settings
  }

  /// Returns the local entry snapshot for `key_id`, defaulting to missing.
  #[must_use]
  pub fn entry(&self, key_id: &str) -> ReplicatorEntry<D> {
    self.entries.get(key_id).cloned().unwrap_or(ReplicatorEntry::missing())
  }

  /// Returns the number of tracked keys.
  #[must_use]
  pub fn entry_count(&self) -> usize {
    self.entries.len()
  }

  /// Handles a get command against the local entry map.
  #[must_use]
  pub fn handle_get<C: Clone>(&self, command: &Get<D, C>) -> ReplicatorOutcome<D, C, S> {
    if !matches!(command.consistency(), ReadConsistency::Local) {
      return ReplicatorOutcome {
        response:      Some(ReplicatorResponse::Get(command.failure())),
        notifications: Vec::new(),
      };
    }

    let response = command.respond_from(&self.entry(command.key().id()));
    ReplicatorOutcome { response: Some(ReplicatorResponse::Get(response)), notifications: Vec::new() }
  }

  /// Handles an update command against the local entry map.
  #[must_use]
  pub fn handle_update<C: Clone, F>(&mut self, command: &Update<D, C>, modify: F) -> ReplicatorOutcome<D, C, S>
  where
    F: FnOnce(Option<&D>) -> Result<D, String>, {
    if !matches!(command.consistency(), WriteConsistency::Local) {
      return ReplicatorOutcome {
        response:      Some(ReplicatorResponse::Update(UpdateResponse::Timeout {
          key:     command.key().clone(),
          request: command.request().cloned(),
        })),
        notifications: Vec::new(),
      };
    }

    let previous = self.entry(command.key().id());
    let (next_entry, response) = command.evaluate(&previous, modify, UpdateWriteOutcome::Success);
    let notifications = if response.is_locally_applied() {
      self.entries.insert(command.key().id().to_string(), next_entry.clone());
      self.notifications_for_entry(command.key().id(), &next_entry)
    } else {
      Vec::new()
    };

    ReplicatorOutcome { response: Some(ReplicatorResponse::Update(response)), notifications }
  }

  /// Handles a delete command against the local entry map.
  #[must_use]
  pub fn handle_delete<C: Clone>(&mut self, command: &Delete<D, C>) -> ReplicatorOutcome<D, C, S> {
    if !matches!(command.consistency(), WriteConsistency::Local) {
      return ReplicatorOutcome {
        response:      Some(ReplicatorResponse::Delete(DeleteResponse::Timeout {
          key:     command.key().clone(),
          request: command.request().cloned(),
        })),
        notifications: Vec::new(),
      };
    }

    let previous = self.entry(command.key().id());
    let (next_entry, response) = command.evaluate(&previous, DeleteWriteOutcome::Success);
    let notifications = if response.is_locally_deleted() {
      self.entries.insert(command.key().id().to_string(), next_entry);
      self.deleted_notifications(command.key().id())
    } else {
      Vec::new()
    };

    ReplicatorOutcome { response: Some(ReplicatorResponse::Delete(response)), notifications }
  }

  /// Registers a subscriber for a key.
  pub fn handle_subscribe(&mut self, command: &Subscribe<D, S>) -> ReplicatorOutcome<D, (), S>
  where
    S: Clone + Ord, {
    self.subscribers.entry(command.key().id().to_string()).or_default().insert(command.subscriber().clone());
    ReplicatorOutcome { response: Some(ReplicatorResponse::Subscribe), notifications: Vec::new() }
  }

  /// Unregisters a subscriber for a key.
  pub fn handle_unsubscribe(&mut self, command: &Unsubscribe<D, S>) -> ReplicatorOutcome<D, (), S>
  where
    S: Clone + Ord + PartialEq, {
    if let Some(subscribers) = self.subscribers.get_mut(command.key().id()) {
      subscribers.remove(command.subscriber());
      if subscribers.is_empty() {
        self.subscribers.remove(command.key().id());
      }
    }
    ReplicatorOutcome { response: Some(ReplicatorResponse::Unsubscribe), notifications: Vec::new() }
  }

  fn notifications_for_entry(&self, key_id: &str, entry: &ReplicatorEntry<D>) -> Vec<(S, SubscribeResponse<D>)>
  where
    S: Clone, {
    let Some(subscribers) = self.subscribers.get(key_id) else {
      return Vec::new();
    };

    match entry {
      | ReplicatorEntry::Present(data) => {
        let key = super::Key::new(key_id);
        subscribers
          .iter()
          .map(|subscriber| (subscriber.clone(), SubscribeResponse::Changed { key: key.clone(), data: data.clone() }))
          .collect()
      },
      | ReplicatorEntry::Deleted => self.deleted_notifications(key_id),
      | ReplicatorEntry::Missing => Vec::new(),
    }
  }

  fn deleted_notifications(&self, key_id: &str) -> Vec<(S, SubscribeResponse<D>)>
  where
    S: Clone, {
    let Some(subscribers) = self.subscribers.get(key_id) else {
      return Vec::new();
    };
    let key = super::Key::new(key_id);
    subscribers.iter().map(|subscriber| (subscriber.clone(), SubscribeResponse::Deleted { key: key.clone() })).collect()
  }
}
