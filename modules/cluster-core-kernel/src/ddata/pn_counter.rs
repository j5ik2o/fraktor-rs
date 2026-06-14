//! Positive-negative counter CRDT.

#[cfg(test)]
#[path = "pn_counter_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};
use core::convert::TryFrom;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CounterArithmeticError, DeltaReplicatedData, GCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  SelfUniqueAddress,
};

const I128_MIN_ABS: u128 = 1_u128 << 127;

/// Counter CRDT supporting both increments and decrements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PNCounter {
  increments: GCounter,
  decrements: GCounter,
}

impl PNCounter {
  /// Creates an empty positive-negative counter.
  #[must_use]
  pub const fn new() -> Self {
    Self::from_parts(GCounter::new(), GCounter::new())
  }

  pub(super) const fn from_parts(increments: GCounter, decrements: GCounter) -> Self {
    Self { increments, decrements }
  }

  pub(super) fn retain_nodes(&self, nodes: &BTreeMap<UniqueAddress, u64>) -> Self {
    Self { increments: self.increments.retain_nodes(nodes), decrements: self.decrements.retain_nodes(nodes) }
  }

  pub(super) fn replace_nodes(&self, nodes: &BTreeMap<UniqueAddress, u64>, replacement: &Self) -> Self {
    Self {
      increments: self.increments.replace_nodes(nodes, &replacement.increments),
      decrements: self.decrements.replace_nodes(nodes, &replacement.decrements),
    }
  }

  pub(super) fn from_node_components(components: BTreeMap<UniqueAddress, (u128, u128)>) -> Self {
    let mut increments = BTreeMap::new();
    let mut decrements = BTreeMap::new();
    for (node, (increment, decrement)) in components {
      if increment != 0 {
        increments.insert(node.clone(), increment);
      }
      if decrement != 0 {
        decrements.insert(node, decrement);
      }
    }

    Self::from_parts(
      GCounter::from_parts(increments, BTreeMap::new()),
      GCounter::from_parts(decrements, BTreeMap::new()),
    )
  }

  pub(super) fn node_components(&self, node: &UniqueAddress) -> (u128, u128) {
    (self.increments.state_value(node), self.decrements.state_value(node))
  }

  pub(super) fn retain_visible_nodes(
    &self,
    dots: &BTreeMap<UniqueAddress, u64>,
    removed_dots: Option<&BTreeMap<UniqueAddress, u64>>,
    own_removed_dots: Option<&BTreeMap<UniqueAddress, u64>>,
    removed_prefix: Option<&Self>,
  ) -> Option<(Self, BTreeMap<UniqueAddress, u64>)> {
    let mut increment_state = BTreeMap::new();
    let mut decrement_state = BTreeMap::new();
    let mut visible_dots = BTreeMap::new();

    for (node, version) in dots {
      let removed_version = removed_dots.and_then(|removed_dots| removed_dots.get(node)).copied();
      let increment = self.increments.state_value(node);
      let decrement = self.decrements.state_value(node);
      let removed_increment =
        removed_prefix.map(|removed_prefix| removed_prefix.increments.state_value(node)).unwrap_or(0);
      let removed_decrement =
        removed_prefix.map(|removed_prefix| removed_prefix.decrements.state_value(node)).unwrap_or(0);
      let owns_covering_remove = own_removed_dots
        .and_then(|own_removed_dots| own_removed_dots.get(node))
        .copied()
        .is_some_and(|own_removed_version| {
          let removed_version = removed_version.unwrap_or(0);
          own_removed_version >= removed_version && *version > own_removed_version
        });

      let (visible_increment, visible_decrement) =
        if removed_version.is_some_and(|removed_version| *version <= removed_version) {
          if increment == removed_increment && decrement == removed_decrement {
            continue;
          }
          (increment, decrement)
        } else if owns_covering_remove {
          (increment, decrement)
        } else if removed_version.is_some() {
          (subtract_removed_prefix(increment, removed_increment), subtract_removed_prefix(decrement, removed_decrement))
        } else {
          (increment, decrement)
        };

      if visible_increment == 0 && visible_decrement == 0 {
        continue;
      }
      if visible_increment != 0 {
        increment_state.insert(node.clone(), visible_increment);
      }
      if visible_decrement != 0 {
        decrement_state.insert(node.clone(), visible_decrement);
      }
      visible_dots.insert(node.clone(), *version);
    }

    if visible_dots.is_empty() {
      None
    } else {
      Some((
        Self::from_parts(
          GCounter::from_parts(increment_state, BTreeMap::new()),
          GCounter::from_parts(decrement_state, BTreeMap::new()),
        ),
        visible_dots,
      ))
    }
  }

  /// Returns a counter with `n` added to the positive component.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the positive component cannot represent the
  /// new value.
  pub fn increment(&self, node: &SelfUniqueAddress, n: u64) -> Result<Self, CounterArithmeticError> {
    Ok(Self { increments: self.increments.increment(node, n)?, decrements: self.decrements.clone() })
  }

  /// Returns a counter with `n` added to the negative component.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the negative component cannot represent the
  /// new value.
  pub fn decrement(&self, node: &SelfUniqueAddress, n: u64) -> Result<Self, CounterArithmeticError> {
    Ok(Self { increments: self.increments.clone(), decrements: self.decrements.increment(node, n)? })
  }

  /// Returns the signed value of this counter.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when either component sum or the signed
  /// difference is out of range.
  pub fn value(&self) -> Result<i128, CounterArithmeticError> {
    signed_difference(self.increments.value()?, self.decrements.value()?)
  }
}

const fn subtract_removed_prefix(current: u128, removed_prefix: u128) -> u128 {
  if current >= removed_prefix { current - removed_prefix } else { current }
}

impl ReplicatedData for PNCounter {
  fn merge(&self, other: &Self) -> Self {
    Self { increments: self.increments.merge(&other.increments), decrements: self.decrements.merge(&other.decrements) }
  }
}

impl DeltaReplicatedData for PNCounter {
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    let increments = self.increments.delta();
    let decrements = self.decrements.delta();

    if increments.is_none() && decrements.is_none() {
      None
    } else {
      Some(Self { increments: increments.unwrap_or_default(), decrements: decrements.unwrap_or_default() })
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    Self {
      increments: self.increments.merge_delta(&delta.increments),
      decrements: self.decrements.merge_delta(&delta.decrements),
    }
  }

  fn reset_delta(&self) -> Self {
    Self { increments: self.increments.reset_delta(), decrements: self.decrements.reset_delta() }
  }
}

impl ReplicatedDelta for PNCounter {
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl RemovedNodePruning for PNCounter {
  type PruneError = CounterArithmeticError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    let mut nodes = self.increments.modified_by_nodes();
    nodes.extend(self.decrements.modified_by_nodes());
    nodes
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.increments.need_pruning_from(removed_node) || self.decrements.need_pruning_from(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    Ok(Self {
      increments: self.increments.prune(removed_node, collapse_into)?,
      decrements: self.decrements.prune(removed_node, collapse_into)?,
    })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    Self {
      increments: self.increments.pruning_cleanup(removed_node),
      decrements: self.decrements.pruning_cleanup(removed_node),
    }
  }
}

impl Default for PNCounter {
  fn default() -> Self {
    Self::new()
  }
}

fn signed_difference(increments: u128, decrements: u128) -> Result<i128, CounterArithmeticError> {
  if increments >= decrements {
    i128::try_from(increments - decrements).map_err(|_| CounterArithmeticError::Overflow)
  } else {
    let diff = decrements - increments;
    if diff == I128_MIN_ABS {
      Ok(i128::MIN)
    } else {
      let magnitude = i128::try_from(diff).map_err(|_| CounterArithmeticError::Overflow)?;
      Ok(-magnitude)
    }
  }
}
