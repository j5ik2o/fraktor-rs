//! Core state machine for distributed pub-sub mediator commands.

#[cfg(test)]
#[path = "distributed_pub_sub_mediator_state_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DistributedPubSubConfig, MediatorAcknowledgement, MediatorCommand, MediatorCommandOutcome, MediatorDeliveryIntent,
  MediatorDeliveryMode, MediatorQuery, MediatorQueryResult, PubSubEnvelope, PubSubError, PubSubNoSubscriberBehavior,
  PubSubPathSemantics, PubSubRoutingMode, PubSubSubscriber, PubSubTopic, SendPathInput, SendToAllPathInput,
  TopicRegistryApplyOutcome, TopicRegistryBucket, TopicRegistryBucketView, TopicRegistryDelta,
  TopicRegistryDeltaCollector, TopicRegistryEntryKind, TopicRegistryStatus,
};

/// Registry-backed mediator state for pub-sub commands.
#[derive(Debug, Clone)]
pub struct DistributedPubSubMediatorState {
  settings:              DistributedPubSubConfig,
  local_owner:           UniqueAddress,
  local_bucket:          TopicRegistryBucket,
  remote_buckets:        BTreeMap<UniqueAddress, TopicRegistryBucket>,
  path_semantics:        PubSubPathSemantics,
  publish_group_cursors: BTreeMap<(PubSubTopic, String), usize>,
  publish_random_cursor: usize,
}

impl DistributedPubSubMediatorState {
  /// Creates mediator state for one local owner.
  #[must_use]
  pub fn new(settings: DistributedPubSubConfig, local_owner: UniqueAddress) -> Self {
    Self {
      local_bucket: TopicRegistryBucket::new(local_owner.clone()),
      path_semantics: PubSubPathSemantics::new(settings.clone(), local_owner.clone()),
      settings,
      local_owner,
      remote_buckets: BTreeMap::new(),
      publish_group_cursors: BTreeMap::new(),
      publish_random_cursor: 0,
    }
  }

  /// Returns mediator settings.
  #[must_use]
  pub const fn settings(&self) -> &DistributedPubSubConfig {
    &self.settings
  }

  /// Returns the local registry owner.
  #[must_use]
  pub const fn local_owner(&self) -> &UniqueAddress {
    &self.local_owner
  }

  /// Returns the local registry bucket.
  #[must_use]
  pub const fn local_bucket(&self) -> &TopicRegistryBucket {
    &self.local_bucket
  }

  /// Rebinds the local owner without dropping existing local or remote registry state.
  pub fn rebind_local_owner(&mut self, local_owner: UniqueAddress) {
    if self.local_owner == local_owner {
      return;
    }
    self.local_owner = local_owner.clone();
    self.local_bucket = self.local_bucket.rebind_owner(local_owner.clone());
    self.remote_buckets.remove(&local_owner);
    self.path_semantics = PubSubPathSemantics::new(self.settings.clone(), local_owner);
  }

  /// Adds or replaces a remote owner bucket snapshot.
  pub fn upsert_remote_bucket(&mut self, bucket: TopicRegistryBucket) {
    if bucket.owner() == &self.local_owner {
      self.remote_buckets.remove(&self.local_owner);
      return;
    }
    self.remote_buckets.insert(bucket.owner().clone(), bucket);
  }

  /// Retains only remote buckets whose owner satisfies the predicate.
  pub fn retain_remote_buckets_by_owner<F>(&mut self, mut retain_owner: F)
  where
    F: FnMut(&UniqueAddress) -> bool, {
    let local_owner = self.local_owner.clone();
    self.remote_buckets.retain(|owner, _| owner != &local_owner && retain_owner(owner));
  }

  /// Applies remote registry delta entries into remote owner buckets.
  #[must_use]
  pub fn apply_delta(
    &mut self,
    delta: &TopicRegistryDelta,
    active_owners: &[UniqueAddress],
  ) -> Vec<TopicRegistryApplyOutcome> {
    let remote_active_owners =
      active_owners.iter().filter(|owner| *owner != &self.local_owner).cloned().collect::<Vec<_>>();
    let mut remote_buckets = self.remote_buckets.values().cloned().collect::<Vec<_>>();
    let outcomes = TopicRegistryDeltaCollector::apply_delta(delta, &mut remote_buckets, &remote_active_owners);
    self.remote_buckets.clear();
    for bucket in remote_buckets {
      self.upsert_remote_bucket(bucket);
    }
    outcomes
  }

  /// Prunes retained removal tombstones after TTL and peer observation checks pass.
  pub fn prune_removed_entries(&mut self, now_millis: u64, peer_statuses: &[TopicRegistryStatus]) {
    let ttl = self.settings.removed_entry_ttl();
    self.local_bucket.prune_removed(now_millis, ttl, peer_statuses);
    for bucket in self.remote_buckets.values_mut() {
      bucket.prune_removed(now_millis, ttl, peer_statuses);
    }
    self.prune_publish_group_cursors();
  }

  /// Applies a validated mediator command and returns the protocol outcome.
  ///
  /// # Errors
  ///
  /// Returns selection errors from path semantics.
  pub fn apply_command(
    &mut self,
    command: MediatorCommand,
    now_millis: u64,
    active_owners: &[UniqueAddress],
  ) -> Result<MediatorCommandOutcome, PubSubError> {
    match command {
      | MediatorCommand::Put { path, target } => {
        self.local_bucket.put_path(path, target);
        Ok(MediatorCommandOutcome::RegistryMutated { version: self.local_bucket.version() })
      },
      | MediatorCommand::Remove { path, target } => {
        self.local_bucket.remove_path(path, target, now_millis);
        Ok(MediatorCommandOutcome::RegistryMutated { version: self.local_bucket.version() })
      },
      | MediatorCommand::Subscribe { topic, group, subscriber } => {
        self.local_bucket.put_subscription(topic.clone(), group.clone(), subscriber.clone());
        Ok(MediatorCommandOutcome::Acknowledged(MediatorAcknowledgement::SubscribeCompleted {
          topic,
          group,
          subscriber,
        }))
      },
      | MediatorCommand::Unsubscribe { topic, group, subscriber } => {
        self.local_bucket.remove_subscription(topic.clone(), group.clone(), subscriber.clone(), now_millis);
        Ok(MediatorCommandOutcome::Acknowledged(MediatorAcknowledgement::UnsubscribeCompleted {
          topic,
          group,
          subscriber,
        }))
      },
      | MediatorCommand::Publish { topic, payload } => {
        Ok(MediatorCommandOutcome::Delivery(self.publish_intent(topic, payload, active_owners)))
      },
      | MediatorCommand::Send { path, payload, local_affinity } => {
        let views = self.delivery_views(active_owners);
        let input = SendPathInput::new(path, payload, local_affinity);
        Ok(MediatorCommandOutcome::Delivery(self.path_semantics.select_send_target(input, &views)))
      },
      | MediatorCommand::SendToAll { path, payload, all_but_self } => {
        let views = self.delivery_views(active_owners);
        let input = SendToAllPathInput::new(path, payload, all_but_self);
        Ok(MediatorCommandOutcome::Delivery(self.path_semantics.select_send_to_all_targets(input, &views)))
      },
      | MediatorCommand::Query(query) => Ok(MediatorCommandOutcome::Query(self.query_result(query, active_owners))),
    }
  }

  /// Returns bucket snapshots for status and delta collection.
  #[must_use]
  pub fn buckets(&self) -> Vec<TopicRegistryBucket> {
    let mut buckets = Vec::with_capacity(self.remote_buckets.len() + 1);
    buckets.push(self.local_bucket.clone());
    buckets.extend(self.remote_buckets.values().cloned());
    buckets
  }

  fn publish_intent(
    &mut self,
    topic: PubSubTopic,
    payload: PubSubEnvelope,
    active_owners: &[UniqueAddress],
  ) -> MediatorDeliveryIntent {
    let mut ungrouped = Vec::new();
    let mut grouped = BTreeMap::<String, Vec<PubSubSubscriber>>::new();
    for view in self.delivery_views(active_owners) {
      for entry in view.entries() {
        if let TopicRegistryEntryKind::TopicSubscription { topic: entry_topic, group, subscriber } = entry.kind()
          && entry_topic == &topic
        {
          if let Some(group) = group {
            grouped.entry(group.clone()).or_default().push(subscriber.clone());
          } else {
            ungrouped.push(subscriber.clone());
          }
        }
      }
    }
    for subscribers in grouped.values_mut() {
      let mut deduplicated = BTreeSet::new();
      subscribers.retain(|subscriber| deduplicated.insert(subscriber.clone()));
    }

    let grouped_targets =
      grouped.into_iter().filter_map(|(group, subscribers)| self.select_group_subscriber(&topic, &group, &subscribers));
    let mut deduplicated = BTreeSet::new();
    let targets = ungrouped
      .into_iter()
      .chain(grouped_targets)
      .filter(|subscriber| deduplicated.insert(subscriber.clone()))
      .collect::<Vec<_>>();

    if targets.is_empty() {
      return self.no_subscriber_topic_intent(topic, payload);
    }
    MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::Publish, targets, payload }
  }

  fn select_group_subscriber(
    &mut self,
    topic: &PubSubTopic,
    group: &str,
    subscribers: &[PubSubSubscriber],
  ) -> Option<PubSubSubscriber> {
    if subscribers.is_empty() {
      return None;
    }
    match self.settings.routing_mode() {
      | PubSubRoutingMode::Random => {
        self.publish_random_cursor = next_random_cursor(self.publish_random_cursor);
        subscribers.get(stable_group_index(topic, group, self.publish_random_cursor, subscribers.len())).cloned()
      },
      | PubSubRoutingMode::RoundRobin => {
        let cursor = self.publish_group_cursors.entry((topic.clone(), String::from(group))).or_insert(0);
        let selected = subscribers.get(*cursor % subscribers.len()).cloned();
        *cursor = cursor.wrapping_add(1);
        selected
      },
    }
  }

  fn query_result(&self, query: MediatorQuery, active_owners: &[UniqueAddress]) -> MediatorQueryResult {
    match query {
      | MediatorQuery::CurrentTopics => {
        MediatorQueryResult::CurrentTopics { topics: self.current_topics(active_owners) }
      },
      | MediatorQuery::SubscriberCount { topic } => {
        let count = self.topic_subscribers(&topic, active_owners).len();
        MediatorQueryResult::SubscriberCount { topic, count }
      },
    }
  }

  fn current_topics(&self, active_owners: &[UniqueAddress]) -> Vec<PubSubTopic> {
    let mut topics = BTreeSet::new();
    for view in self.delivery_views(active_owners) {
      for entry in view.entries() {
        if let TopicRegistryEntryKind::TopicSubscription { topic, .. } = entry.kind() {
          topics.insert(topic.clone());
        }
      }
    }
    topics.into_iter().collect()
  }

  fn topic_subscribers(&self, topic: &PubSubTopic, active_owners: &[UniqueAddress]) -> Vec<PubSubSubscriber> {
    let mut deduplicated = BTreeSet::new();
    self
      .delivery_views(active_owners)
      .into_iter()
      .flat_map(|view| view.entries().to_vec())
      .filter_map(|entry| match entry.kind() {
        | TopicRegistryEntryKind::TopicSubscription { topic: entry_topic, subscriber, .. } if entry_topic == topic => {
          deduplicated.insert(subscriber.clone()).then(|| subscriber.clone())
        },
        | _ => None,
      })
      .collect()
  }

  fn prune_publish_group_cursors(&mut self) {
    let live_topics = self.live_topics();
    self.publish_group_cursors.retain(|(topic, _group), _| live_topics.contains(topic));
  }

  fn live_topics(&self) -> BTreeSet<PubSubTopic> {
    self
      .buckets()
      .into_iter()
      .flat_map(|bucket| bucket.entries())
      .filter_map(|entry| match entry.kind() {
        | TopicRegistryEntryKind::TopicSubscription { topic, .. } => Some(topic.clone()),
        | TopicRegistryEntryKind::Path { .. } | TopicRegistryEntryKind::Removed { .. } => None,
      })
      .collect()
  }

  fn delivery_views(&self, active_owners: &[UniqueAddress]) -> Vec<TopicRegistryBucketView> {
    let mut views = Vec::with_capacity(self.remote_buckets.len() + 1);
    views.push(self.local_bucket.delivery_view(active_owners));
    views.extend(self.remote_buckets.values().map(|bucket| bucket.delivery_view(active_owners)));
    views
  }

  const fn no_subscriber_topic_intent(&self, topic: PubSubTopic, payload: PubSubEnvelope) -> MediatorDeliveryIntent {
    match self.settings.no_subscriber_behavior() {
      | PubSubNoSubscriberBehavior::Drop => MediatorDeliveryIntent::DroppedTopic { topic, payload },
      | PubSubNoSubscriberBehavior::DeadLetter => MediatorDeliveryIntent::DeadLetterTopic { topic, payload },
    }
  }
}

const fn next_random_cursor(current: usize) -> usize {
  current.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn stable_group_index(topic: &PubSubTopic, group: &str, cursor: usize, len: usize) -> usize {
  if len == 0 {
    return 0;
  }
  let topic_accumulator =
    topic.as_str().as_bytes().iter().fold(cursor, |accumulator, byte| accumulator.wrapping_add(usize::from(*byte)));
  group
    .as_bytes()
    .iter()
    .fold(topic_accumulator.wrapping_add(31), |accumulator, byte| accumulator.wrapping_add(usize::from(*byte)))
    % len
}
