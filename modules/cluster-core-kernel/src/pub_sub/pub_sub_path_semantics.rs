//! Path target selection semantics for distributed pub-sub mediator commands.

#[cfg(test)]
#[path = "pub_sub_path_semantics_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  vec,
  vec::Vec,
};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DistributedPubSubSettings, MediatorDeliveryIntent, MediatorDeliveryMode, MediatorPathKey, PubSubEnvelope,
  PubSubNoSubscriberBehavior, PubSubRoutingMode, PubSubSubscriber, SendPathInput, SendToAllPathInput,
  TopicRegistryBucketView, TopicRegistryEntryKind,
};

/// Selects path delivery targets from topic registry bucket views.
#[derive(Debug, Clone)]
pub struct PubSubPathSemantics {
  settings:            DistributedPubSubSettings,
  local_owner:         UniqueAddress,
  round_robin_cursors: BTreeMap<MediatorPathKey, usize>,
  random_cursor:       usize,
}

impl PubSubPathSemantics {
  /// Creates a path selector.
  #[must_use]
  pub const fn new(settings: DistributedPubSubSettings, local_owner: UniqueAddress) -> Self {
    Self { settings, local_owner, round_robin_cursors: BTreeMap::new(), random_cursor: 0 }
  }

  /// Selects one matching path target for `Send`.
  #[must_use]
  pub fn select_send_target(
    &mut self,
    input: SendPathInput,
    buckets: &[TopicRegistryBucketView],
  ) -> MediatorDeliveryIntent {
    let mut candidates = Self::matching_targets(&input.path, buckets);
    if input.local_affinity {
      let local_candidates =
        candidates.iter().filter(|candidate| candidate.0 == self.local_owner).cloned().collect::<Vec<_>>();
      if !local_candidates.is_empty() {
        candidates = local_candidates;
      }
    }
    self.prune_round_robin_cursors(buckets);
    let Some(target) = self.select_one(&input.path, &candidates) else {
      return self.no_subscriber_intent(input.path, input.payload);
    };
    MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Send,
      targets: vec![target.1],
      payload: input.payload,
    }
  }

  /// Selects all matching path targets for `SendToAll`.
  #[must_use]
  pub fn select_send_to_all_targets(
    &self,
    input: SendToAllPathInput,
    buckets: &[TopicRegistryBucketView],
  ) -> MediatorDeliveryIntent {
    let mut deduplicated = BTreeSet::new();
    let targets = Self::matching_targets(&input.path, buckets)
      .into_iter()
      .filter(|candidate| !input.all_but_self || candidate.0 != self.local_owner)
      .map(|candidate| candidate.1)
      .filter(|subscriber| deduplicated.insert(subscriber.clone()))
      .collect::<Vec<_>>();
    if targets.is_empty() {
      return self.no_subscriber_intent(input.path, input.payload);
    }
    MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::SendToAll, targets, payload: input.payload }
  }

  fn select_one(
    &mut self,
    path: &MediatorPathKey,
    candidates: &[(UniqueAddress, PubSubSubscriber)],
  ) -> Option<(UniqueAddress, PubSubSubscriber)> {
    if candidates.is_empty() {
      return None;
    }
    match self.settings.routing_mode() {
      | PubSubRoutingMode::Random => {
        self.random_cursor = next_random_cursor(self.random_cursor);
        candidates.get(stable_index(path.as_str(), self.random_cursor, candidates.len())).cloned()
      },
      | PubSubRoutingMode::RoundRobin => {
        let cursor = self.round_robin_cursors.entry(path.clone()).or_insert(0);
        let selected = candidates.get(*cursor % candidates.len()).cloned();
        *cursor = cursor.wrapping_add(1);
        selected
      },
    }
  }

  fn matching_targets(
    path: &MediatorPathKey,
    buckets: &[TopicRegistryBucketView],
  ) -> Vec<(UniqueAddress, PubSubSubscriber)> {
    let mut candidates = Vec::new();
    for bucket in buckets {
      if !bucket.is_delivery_candidate() {
        continue;
      }
      for entry in bucket.entries() {
        if let TopicRegistryEntryKind::Path { path: entry_path, target } = entry.kind()
          && entry_path == path
        {
          candidates.push((bucket.owner().clone(), target.clone()));
        }
      }
    }
    candidates
  }

  fn prune_round_robin_cursors(&mut self, buckets: &[TopicRegistryBucketView]) {
    self.round_robin_cursors.retain(|path, _| {
      buckets.iter().any(|bucket| {
        bucket.is_delivery_candidate()
          && bucket.entries().iter().any(
            |entry| matches!(entry.kind(), TopicRegistryEntryKind::Path { path: entry_path, .. } if entry_path == path),
          )
      })
    });
  }

  const fn no_subscriber_intent(&self, path: MediatorPathKey, payload: PubSubEnvelope) -> MediatorDeliveryIntent {
    match self.settings.no_subscriber_behavior() {
      | PubSubNoSubscriberBehavior::Drop => MediatorDeliveryIntent::Dropped { path, payload },
      | PubSubNoSubscriberBehavior::DeadLetter => MediatorDeliveryIntent::DeadLetter { path, payload },
    }
  }
}

const fn next_random_cursor(current: usize) -> usize {
  current.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn stable_index(key: &str, cursor: usize, len: usize) -> usize {
  if len == 0 {
    return 0;
  }
  key.as_bytes().iter().fold(cursor, |accumulator, byte| accumulator.wrapping_add(usize::from(*byte))) % len
}
