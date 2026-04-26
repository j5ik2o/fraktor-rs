//! Pure state type backing the remote watcher.

#[cfg(test)]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::fmt::{Debug, Formatter, Result as FmtResult};

use ahash::RandomState;
use fraktor_actor_core_rs::core::kernel::actor::actor_path::ActorPath;
use hashbrown::{HashMap, hash_map::Entry};

use crate::core::{
  address::Address,
  failure_detector::PhiAccrualFailureDetector,
  watcher::{watcher_command::WatcherCommand, watcher_effect::WatcherEffect},
};

/// Type alias for the deterministic hasher used across every map in this
/// module. Phase A does not need a cryptographic hasher; `ahash` gives us a
/// `no_std`-compatible deterministic `BuildHasher`.
type Map<K, V> = HashMap<K, V, RandomState>;

/// Factory used by [`WatcherState`] to create a fresh
/// [`PhiAccrualFailureDetector`] on demand when a new remote node is
/// encountered.
type DetectorFactory = fn(&Address) -> PhiAccrualFailureDetector;

/// Pure state portion of the remote watcher.
///
/// `WatcherState` tracks which local actors are watching which remote actors
/// and runs a per-node [`PhiAccrualFailureDetector`]. It is deliberately
/// asynchronous-runtime-free: the adapter layer drives it with
/// [`WatcherCommand`] values and applies the returned
/// [`WatcherEffect`]s.
pub struct WatcherState {
  /// Per-target → set of watchers.
  watching:         Map<ActorPath, Vec<ActorPath>>,
  /// Per-remote-node → set of targets hosted on that node.
  targets_by_node:  Map<Address, Vec<ActorPath>>,
  /// Per-remote-node failure detector.
  detectors:        Map<Address, PhiAccrualFailureDetector>,
  /// Remote nodes that have already been notified as terminated during the
  /// current session, so we do not re-emit effects on every tick.
  already_notified: Map<Address, ()>,
  /// Factory used to build a fresh detector when a new remote node appears.
  detector_factory: DetectorFactory,
}

impl Debug for WatcherState {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("WatcherState")
      .field("watching", &self.watching.len())
      .field("targets_by_node", &self.targets_by_node.len())
      .field("detectors", &self.detectors.len())
      .finish_non_exhaustive()
  }
}

impl WatcherState {
  /// Creates a new [`WatcherState`] using `detector_factory` to produce a
  /// fresh [`PhiAccrualFailureDetector`] on every newly observed remote node.
  #[must_use]
  pub fn new(detector_factory: DetectorFactory) -> Self {
    Self {
      watching: Map::with_hasher(RandomState::new()),
      targets_by_node: Map::with_hasher(RandomState::new()),
      detectors: Map::with_hasher(RandomState::new()),
      already_notified: Map::with_hasher(RandomState::new()),
      detector_factory,
    }
  }

  /// Returns the number of remote nodes currently being observed.
  #[must_use]
  pub fn node_count(&self) -> usize {
    self.targets_by_node.len()
  }

  /// Returns the total number of (target, watcher) pairs currently tracked.
  #[must_use]
  pub fn watch_pair_count(&self) -> usize {
    self.watching.values().map(Vec::len).sum()
  }

  /// Applies a command and returns the list of effects to perform.
  pub fn handle(&mut self, command: WatcherCommand) -> Vec<WatcherEffect> {
    match command {
      | WatcherCommand::Watch { target, watcher } => self.on_watch(target, watcher),
      | WatcherCommand::Unwatch { target, watcher } => self.on_unwatch(&target, &watcher),
      | WatcherCommand::HeartbeatReceived { from, now } => self.on_heartbeat(&from, now),
      | WatcherCommand::HeartbeatTick { now } => self.on_tick(now),
    }
  }

  // -------------------------------------------------------------------------
  // command handlers
  // -------------------------------------------------------------------------

  fn on_watch(&mut self, target: ActorPath, watcher: ActorPath) -> Vec<WatcherEffect> {
    // Only remote targets are tracked; local paths are silently ignored.
    let Some(node) = address_from_path(&target) else {
      return Vec::new();
    };

    // Register (target -> watcher) pair.
    let entry = self.watching.entry(target.clone()).or_default();
    if !entry.contains(&watcher) {
      entry.push(watcher);
    }

    // Register target under its hosting node.
    let node_targets = self.targets_by_node.entry(node.clone()).or_default();
    if !node_targets.contains(&target) {
      node_targets.push(target);
    }

    self.ensure_detector(&node);

    // Emit an initial heartbeat towards the new peer so that it can respond
    // and make itself observable.
    alloc::vec![WatcherEffect::SendHeartbeat { to: node }]
  }

  fn on_unwatch(&mut self, target: &ActorPath, watcher: &ActorPath) -> Vec<WatcherEffect> {
    if let Some(watchers) = self.watching.get_mut(target) {
      watchers.retain(|w| w != watcher);
      if watchers.is_empty() {
        self.watching.remove(target);
        // Also remove target from its hosting node map.
        if let Some(node) = address_from_path(target)
          && let Some(targets) = self.targets_by_node.get_mut(&node)
        {
          targets.retain(|t| t != target);
          if targets.is_empty() {
            self.targets_by_node.remove(&node);
            self.detectors.remove(&node);
            self.already_notified.remove(&node);
          }
        }
      }
    }
    Vec::new()
  }

  fn on_heartbeat(&mut self, from: &Address, now: u64) -> Vec<WatcherEffect> {
    // 監視対象外ノードからの heartbeat は detector の無制限肥大を防ぐため無視する。
    if !self.targets_by_node.contains_key(from) {
      return Vec::new();
    }
    // 通知済みフラグを消して、再度沈黙した場合に検出できるようにする。
    self.already_notified.remove(from);
    self.ensure_detector(from).heartbeat(now);
    Vec::new()
  }

  fn on_tick(&mut self, now: u64) -> Vec<WatcherEffect> {
    let mut effects = Vec::new();

    // Periodic heartbeat towards every observed node.
    for node in self.targets_by_node.keys() {
      effects.push(WatcherEffect::SendHeartbeat { to: node.clone() });
    }

    // Failure-detector evaluation.
    let mut unavailable_nodes: Vec<Address> = Vec::new();
    for (node, detector) in &self.detectors {
      if !detector.is_available(now) && !self.already_notified.contains_key(node) {
        unavailable_nodes.push(node.clone());
      }
    }

    for node in unavailable_nodes {
      self.already_notified.insert(node.clone(), ());
      if let Some(targets) = self.targets_by_node.get(&node) {
        for target in targets.clone() {
          if let Some(watchers) = self.watching.get(&target) {
            effects.push(WatcherEffect::NotifyTerminated { target: target.clone(), watchers: watchers.clone() });
          }
        }
      }
      effects.push(WatcherEffect::NotifyQuarantined { node });
    }

    effects
  }

  fn ensure_detector(&mut self, node: &Address) -> &mut PhiAccrualFailureDetector {
    match self.detectors.entry(node.clone()) {
      | Entry::Occupied(entry) => entry.into_mut(),
      | Entry::Vacant(entry) => entry.insert((self.detector_factory)(node)),
    }
  }
}

/// Extracts the remote [`Address`] from an [`ActorPath`], returning `None` if
/// the path has no authority component (i.e. it addresses a local actor).
fn address_from_path(path: &ActorPath) -> Option<Address> {
  let parts = path.parts();
  let endpoint = parts.authority_endpoint()?;
  // endpoint is "host" or "host:port"
  let (host, port) = match endpoint.rfind(':') {
    | Some(idx) => {
      let host_part = &endpoint[..idx];
      let port_part = &endpoint[idx + 1..];
      match port_part.parse::<u16>() {
        | Ok(p) => (host_part.to_string(), p),
        | Err(_) => (endpoint.clone(), 0),
      }
    },
    | None => (endpoint, 0),
  };
  let system: String = parts.system().to_string();
  Some(Address::new(system, host, port))
}
