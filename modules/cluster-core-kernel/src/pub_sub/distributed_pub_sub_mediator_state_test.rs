use alloc::{string::String, vec};
use core::{slice::from_ref, time::Duration};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    DistributedPubSubConfig, DistributedPubSubMediatorState, MediatorCommand, MediatorCommandOutcome,
    MediatorDeliveryIntent, MediatorDeliveryMode, MediatorPathKey, MediatorQueryResult, PubSubEnvelope,
    PubSubNoSubscriberBehavior, PubSubRoutingMode, PubSubSubscriber, PubSubTopic, TopicRegistryBucket,
    TopicRegistryDelta, TopicRegistryDeltaEntry, TopicRegistryEntry, TopicRegistryEntryKey, TopicRegistryEntryKind,
    TopicRegistryStatus, TopicRegistryVersion,
  },
};

fn owner(name: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", name, 2552), uid)
}

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

fn payload() -> PubSubEnvelope {
  PubSubEnvelope { serializer_id: 41, type_name: String::from("example.Message"), bytes: vec![1, 2, 3] }
}

fn publish_targets(outcome: MediatorCommandOutcome) -> Vec<PubSubSubscriber> {
  match outcome {
    | MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver { targets, .. }) => targets,
    | _ => panic!("unexpected publish outcome"),
  }
}

fn config(behavior: PubSubNoSubscriberBehavior) -> DistributedPubSubConfig {
  DistributedPubSubConfig::try_new(
    None,
    PubSubRoutingMode::RoundRobin,
    Duration::from_secs(1),
    Duration::from_secs(30),
    100,
    behavior,
  )
  .expect("config")
}

fn random_config(behavior: PubSubNoSubscriberBehavior) -> DistributedPubSubConfig {
  DistributedPubSubConfig::try_new(
    None,
    PubSubRoutingMode::Random,
    Duration::from_secs(1),
    Duration::from_secs(30),
    100,
    behavior,
  )
  .expect("config")
}

#[test]
fn subscribe_publish_unsubscribe_and_query_use_registry_entries() {
  let local = owner("node-a", 1);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let first = subscriber("sub-1");
  let second = subscriber("sub-2");
  let active = from_ref(&local);

  let subscribed = state
    .apply_command(MediatorCommand::try_subscribe(topic.clone(), None, first.clone()).expect("subscribe"), 10, active)
    .expect("subscribed");
  assert!(matches!(subscribed, MediatorCommandOutcome::Acknowledged(_)));
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), second.clone()).expect("subscribe"),
      11,
      active,
    )
    .expect("group subscribed");

  let published = state
    .apply_command(MediatorCommand::try_publish(topic.clone(), payload()).expect("publish"), 12, active)
    .expect("published");
  match published {
    | MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver { mode, targets, .. }) => {
      assert_eq!(mode, MediatorDeliveryMode::Publish);
      assert_eq!(targets, vec![first.clone(), second.clone()]);
    },
    | _ => panic!("unexpected publish outcome"),
  }

  let count =
    state.apply_command(MediatorCommand::subscriber_count(topic.clone()).expect("query"), 13, active).expect("count");
  assert_eq!(
    count,
    MediatorCommandOutcome::Query(MediatorQueryResult::SubscriberCount { topic: topic.clone(), count: 2 })
  );

  state
    .apply_command(
      MediatorCommand::try_unsubscribe(topic.clone(), None, first.clone()).expect("unsubscribe"),
      14,
      active,
    )
    .expect("unsubscribed");
  let key = TopicRegistryEntryKey::TopicSubscription { topic, group: None, subscriber: first };
  assert!(matches!(state.local_bucket().entry(&key).expect("tombstone").kind(), TopicRegistryEntryKind::Removed {
    removed_at_millis: 14,
  }));
}

#[test]
fn grouped_publish_selects_one_subscriber_per_group() {
  let local = owner("node-a", 1);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let active = from_ref(&local);

  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), subscriber("sub-1"))
        .expect("subscribe"),
      10,
      active,
    )
    .expect("subscribed");
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), subscriber("sub-2"))
        .expect("subscribe"),
      11,
      active,
    )
    .expect("subscribed");

  let published = state
    .apply_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 12, active)
    .expect("published");

  assert_eq!(
    published,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![subscriber("sub-1")],
      payload: payload(),
    })
  );
}

#[test]
fn grouped_publish_round_robins_within_group() {
  let local = owner("node-a", 1);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let first = subscriber("sub-1");
  let second = subscriber("sub-2");
  let active = from_ref(&local);

  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), first.clone()).expect("subscribe"),
      10,
      active,
    )
    .expect("subscribed");
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), second.clone()).expect("subscribe"),
      11,
      active,
    )
    .expect("subscribed");

  let first_publish = state
    .apply_command(MediatorCommand::try_publish(topic.clone(), payload()).expect("publish"), 12, active)
    .expect("published");
  let second_publish = state
    .apply_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 13, active)
    .expect("published");

  assert_eq!(
    first_publish,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![first],
      payload: payload(),
    })
  );
  assert_eq!(
    second_publish,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![second],
      payload: payload(),
    })
  );
}

#[test]
fn grouped_publish_deduplicates_subscribers_before_group_selection() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let group = Some(String::from("blue"));
  let first = subscriber("sub-1");
  let second = subscriber("sub-2");
  let active = vec![local.clone(), remote.clone()];

  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), group.clone(), first.clone()).expect("subscribe"),
      10,
      &active,
    )
    .expect("subscribed");
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), group.clone(), second.clone()).expect("subscribe"),
      11,
      &active,
    )
    .expect("subscribed");
  let mut remote_bucket = TopicRegistryBucket::new(remote);
  remote_bucket.put_subscription(topic.clone(), group, first.clone());
  state.upsert_remote_bucket(remote_bucket);

  let targets = (0..4)
    .map(|time| {
      let published = state
        .apply_command(MediatorCommand::try_publish(topic.clone(), payload()).expect("publish"), 12 + time, &active)
        .expect("published");
      publish_targets(published)
    })
    .collect::<Vec<_>>();

  assert_eq!(targets, vec![vec![first.clone()], vec![second.clone()], vec![first], vec![second]]);
}

#[test]
fn prune_removed_entries_drops_publish_group_cursors_for_removed_topics() {
  let local = owner("node-a", 1);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let group = String::from("blue");
  let first = subscriber("sub-1");
  let second = subscriber("sub-2");
  let active = from_ref(&local);

  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(group.clone()), first.clone()).expect("subscribe"),
      10,
      active,
    )
    .expect("subscribed");
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), Some(group.clone()), second.clone()).expect("subscribe"),
      11,
      active,
    )
    .expect("subscribed");
  state
    .apply_command(MediatorCommand::try_publish(topic.clone(), payload()).expect("publish"), 12, active)
    .expect("published");
  assert!(state.publish_group_cursors.contains_key(&(topic.clone(), group.clone())));

  state
    .apply_command(
      MediatorCommand::try_unsubscribe(topic.clone(), Some(group.clone()), first).expect("unsubscribe"),
      13,
      active,
    )
    .expect("unsubscribed");
  state
    .apply_command(
      MediatorCommand::try_unsubscribe(topic.clone(), Some(group.clone()), second).expect("unsubscribe"),
      14,
      active,
    )
    .expect("unsubscribed");
  let expired_at = 14 + state.config().removed_entry_ttl().as_millis() as u64;
  state.prune_removed_entries(expired_at, &[]);

  assert!(!state.publish_group_cursors.contains_key(&(topic, group)));
}

#[test]
fn random_group_publish_uses_topic_in_selection_key() {
  let local = owner("node-a", 1);
  let first = subscriber("sub-1");
  let second = subscriber("sub-2");
  let active = from_ref(&local);
  let mut a_state = DistributedPubSubMediatorState::new(random_config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let mut b_state = DistributedPubSubMediatorState::new(random_config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let a_topic = PubSubTopic::new("a");
  let b_topic = PubSubTopic::new("b");

  for state_topic in [(&mut a_state, a_topic.clone()), (&mut b_state, b_topic.clone())] {
    let (state, topic) = state_topic;
    state
      .apply_command(
        MediatorCommand::try_subscribe(topic.clone(), Some(String::from("blue")), first.clone()).expect("subscribe"),
        10,
        active,
      )
      .expect("subscribed");
    state
      .apply_command(
        MediatorCommand::try_subscribe(topic, Some(String::from("blue")), second.clone()).expect("subscribe"),
        11,
        active,
      )
      .expect("subscribed");
  }

  let a_publish = a_state
    .apply_command(MediatorCommand::try_publish(a_topic, payload()).expect("publish"), 12, active)
    .expect("published");
  let b_publish = b_state
    .apply_command(MediatorCommand::try_publish(b_topic, payload()).expect("publish"), 12, active)
    .expect("published");

  assert_eq!(
    a_publish,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![second],
      payload: payload(),
    })
  );
  assert_eq!(
    b_publish,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![first],
      payload: payload(),
    })
  );
}

#[test]
fn publish_no_subscriber_uses_configured_topic_intent() {
  let local = owner("node-a", 1);
  let topic = PubSubTopic::new("missing");
  let mut drop_state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let mut dead_letter_state =
    DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::DeadLetter), local.clone());

  let dropped = drop_state
    .apply_command(MediatorCommand::try_publish(topic.clone(), payload()).expect("publish"), 10, from_ref(&local))
    .expect("drop");
  let dead_letter = dead_letter_state
    .apply_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 10, from_ref(&local))
    .expect("dead");

  assert!(matches!(dropped, MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::DroppedTopic { .. })));
  assert!(matches!(dead_letter, MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::DeadLetterTopic { .. })));
}

#[test]
fn subscriber_count_deduplicates_same_subscriber_across_buckets() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let topic = PubSubTopic::new("news");
  let duplicate = subscriber("same-target");
  let active = vec![local.clone(), remote.clone()];

  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), None, duplicate.clone()).expect("subscribe"),
      10,
      &active,
    )
    .expect("local subscribed");

  let mut remote_bucket = TopicRegistryBucket::new(remote);
  remote_bucket.put_subscription(topic.clone(), None, duplicate);
  state.upsert_remote_bucket(remote_bucket);

  let count =
    state.apply_command(MediatorCommand::subscriber_count(topic.clone()).expect("query"), 11, &active).expect("count");

  assert_eq!(count, MediatorCommandOutcome::Query(MediatorQueryResult::SubscriberCount { topic, count: 1 }));
}

#[test]
fn count_returns_total_active_subscriber_registrations() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let inactive = owner("node-c", 3);
  let news = PubSubTopic::new("news");
  let metrics = PubSubTopic::new("metrics");
  let local_subscriber = subscriber("same-target");
  let remote_subscriber = subscriber("remote-target");
  let active = vec![local.clone(), remote.clone()];
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());

  state
    .apply_command(
      MediatorCommand::try_subscribe(news.clone(), None, local_subscriber.clone()).expect("subscribe"),
      10,
      &active,
    )
    .expect("news subscribed");
  state
    .apply_command(
      MediatorCommand::try_subscribe(metrics.clone(), None, local_subscriber.clone()).expect("subscribe"),
      11,
      &active,
    )
    .expect("metrics subscribed");

  let mut remote_bucket = TopicRegistryBucket::new(remote);
  remote_bucket.put_subscription(news, None, local_subscriber);
  remote_bucket.put_subscription(metrics, Some(String::from("blue")), remote_subscriber);
  state.upsert_remote_bucket(remote_bucket);

  let mut inactive_bucket = TopicRegistryBucket::new(inactive);
  inactive_bucket.put_subscription(PubSubTopic::new("ignored"), None, subscriber("inactive"));
  state.upsert_remote_bucket(inactive_bucket);

  let count = state.apply_command(MediatorCommand::count(), 12, &active).expect("count");

  assert_eq!(count, MediatorCommandOutcome::Query(MediatorQueryResult::Count { count: 3 }));
}

#[test]
fn count_includes_active_path_registrations() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let inactive = owner("node-c", 3);
  let path = MediatorPathKey::parse("fraktor://sys/user/service").expect("path");
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let active = vec![local.clone(), remote.clone()];

  state
    .apply_command(
      MediatorCommand::try_put("fraktor://sys/user/service", subscriber("local-path-target")).expect("put"),
      10,
      &active,
    )
    .expect("put");

  let mut remote_bucket = TopicRegistryBucket::new(remote);
  remote_bucket.put_path(path.clone(), subscriber("remote-path-target"));
  state.upsert_remote_bucket(remote_bucket);

  let mut inactive_bucket = TopicRegistryBucket::new(inactive);
  inactive_bucket.put_path(path, subscriber("inactive-path-target"));
  state.upsert_remote_bucket(inactive_bucket);

  let count = state.apply_command(MediatorCommand::count(), 11, &active).expect("count");

  assert_eq!(count, MediatorCommandOutcome::Query(MediatorQueryResult::Count { count: 2 }));
}

#[test]
fn publish_and_send_use_separate_registry_namespaces() {
  let local = owner("node-a", 1);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let target = subscriber("path-target");
  let topic = PubSubTopic::new("news");
  let active = from_ref(&local);

  state
    .apply_command(MediatorCommand::try_put("fraktor://sys/user/news", target.clone()).expect("put"), 10, active)
    .expect("put");
  state
    .apply_command(
      MediatorCommand::try_subscribe(topic.clone(), None, subscriber("topic-target")).expect("subscribe"),
      11,
      active,
    )
    .expect("subscribe");

  let sent = state
    .apply_command(MediatorCommand::try_send("fraktor://sys/user/news", payload(), false).expect("send"), 12, active)
    .expect("sent");
  let published = state
    .apply_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 13, active)
    .expect("published");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Send,
      targets: vec![target],
      payload: payload(),
    })
  );
  assert!(matches!(
    published,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::Publish, .. })
  ));
}

#[test]
fn local_owner_is_not_delivery_candidate_when_active_owners_exclude_it() {
  let local = owner("advertised", 1);
  let membership_owner = owner("member", 2);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());

  state
    .apply_command(
      MediatorCommand::try_put("fraktor://sys/user/service", subscriber("path-target")).expect("put"),
      10,
      from_ref(&membership_owner),
    )
    .expect("put");

  let sent = state
    .apply_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload(), false).expect("send"),
      11,
      from_ref(&membership_owner),
    )
    .expect("send");

  assert!(matches!(sent, MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Dropped { .. })));
}

#[test]
fn rebinding_local_owner_preserves_existing_registry_entries() {
  let first_owner = owner("node-a", 1);
  let second_owner = owner("node-b", 2);
  let target = subscriber("path-target");
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), first_owner);

  state
    .apply_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&second_owner),
    )
    .expect("put");
  state.rebind_local_owner(second_owner.clone());

  let sent = state
    .apply_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload(), false).expect("send"),
      11,
      from_ref(&second_owner),
    )
    .expect("send");

  assert_eq!(state.local_owner(), &second_owner);
  assert_eq!(state.local_bucket().version().value(), 1);
  assert!(matches!(sent, MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver { .. })));
}

#[test]
fn rebinding_local_owner_removes_remote_bucket_with_same_owner() {
  let placeholder_owner = owner("advertised", 1);
  let real_owner = owner("member", 2);
  let local_target = subscriber("local-path-target");
  let remote_target = subscriber("stale-remote-target");
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), placeholder_owner);
  let active = from_ref(&real_owner);

  state
    .apply_command(
      MediatorCommand::try_put("fraktor://sys/user/service", local_target.clone()).expect("put"),
      10,
      active,
    )
    .expect("put");
  let mut remote_bucket = TopicRegistryBucket::new(real_owner.clone());
  remote_bucket.put_path(MediatorPathKey::parse("fraktor://sys/user/service").expect("path"), remote_target);
  state.upsert_remote_bucket(remote_bucket);

  state.rebind_local_owner(real_owner.clone());
  let sent = state
    .apply_command(MediatorCommand::try_send("fraktor://sys/user/service", payload(), false).expect("send"), 11, active)
    .expect("send");

  let owner_bucket_count = state.buckets().into_iter().filter(|bucket| bucket.owner() == &real_owner).count();
  assert_eq!(owner_bucket_count, 1);
  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Send,
      targets: vec![local_target],
      payload: payload(),
    })
  );
}

#[test]
fn upsert_remote_bucket_discards_bucket_for_local_owner() {
  let local = owner("node-a", 1);
  let local_target = subscriber("local-path-target");
  let remote_target = subscriber("stale-remote-target");
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let active = from_ref(&local);

  state
    .apply_command(
      MediatorCommand::try_put("fraktor://sys/user/service", local_target.clone()).expect("put"),
      10,
      active,
    )
    .expect("put");
  let mut remote_bucket = TopicRegistryBucket::new(local.clone());
  remote_bucket.put_path(MediatorPathKey::parse("fraktor://sys/user/service").expect("path"), remote_target);
  state.upsert_remote_bucket(remote_bucket);

  let sent = state
    .apply_command(MediatorCommand::try_send("fraktor://sys/user/service", payload(), false).expect("send"), 11, active)
    .expect("send");

  let owner_bucket_count = state.buckets().into_iter().filter(|bucket| bucket.owner() == &local).count();
  assert_eq!(owner_bucket_count, 1);
  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Send,
      targets: vec![local_target],
      payload: payload(),
    })
  );
}

#[test]
fn remote_bucket_is_ignored_when_owner_is_not_active() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  remote_bucket.put_subscription(PubSubTopic::new("news"), None, subscriber("remote"));
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  state.upsert_remote_bucket(remote_bucket);

  let published = state
    .apply_command(
      MediatorCommand::try_publish(PubSubTopic::new("news"), payload()).expect("publish"),
      10,
      from_ref(&local),
    )
    .expect("published");

  assert!(matches!(published, MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::DroppedTopic { .. })));
}

#[test]
fn remote_delta_is_applied_to_delivery_registry() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let topic = PubSubTopic::new("news");
  let remote_subscriber = subscriber("remote");
  let active = vec![local.clone(), remote.clone()];
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local);
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: remote_subscriber.clone(),
  };
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: remote_subscriber.clone(),
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(remote, key, entry)]);

  let outcomes = state.apply_delta(&delta, &active);
  let published = state
    .apply_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 10, &active)
    .expect("published");

  assert!(matches!(outcomes.as_slice(), [crate::pub_sub::TopicRegistryApplyOutcome::Applied { .. }]));
  assert_eq!(
    published,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![remote_subscriber],
      payload: payload(),
    })
  );
}

#[test]
fn retain_remote_buckets_by_owner_removes_inactive_status_buckets() {
  let local = owner("node-a", 1);
  let active_remote = owner("node-b", 2);
  let inactive_remote = owner("node-c", 3);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local);
  state.upsert_remote_bucket(TopicRegistryBucket::new(active_remote.clone()));
  state.upsert_remote_bucket(TopicRegistryBucket::new(inactive_remote.clone()));

  state.retain_remote_buckets_by_owner(|owner| owner == &active_remote);

  let bucket_owners = state.buckets().into_iter().map(|bucket| bucket.owner().clone()).collect::<Vec<_>>();
  assert!(bucket_owners.contains(&active_remote));
  assert!(!bucket_owners.contains(&inactive_remote));
}

#[test]
fn prune_removed_entries_uses_config_ttl_and_peer_status_observation() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let mut state = DistributedPubSubMediatorState::new(config(PubSubNoSubscriberBehavior::Drop), local.clone());
  let local_topic = PubSubTopic::new("local-news");
  let local_subscriber = subscriber("local-target");
  let remote_topic = PubSubTopic::new("remote-news");
  let remote_subscriber = subscriber("remote-target");
  let local_key = TopicRegistryEntryKey::TopicSubscription {
    topic:      local_topic.clone(),
    group:      None,
    subscriber: local_subscriber.clone(),
  };
  let remote_key = TopicRegistryEntryKey::TopicSubscription {
    topic:      remote_topic.clone(),
    group:      None,
    subscriber: remote_subscriber.clone(),
  };
  let active = from_ref(&local);

  state
    .apply_command(
      MediatorCommand::try_subscribe(local_topic.clone(), None, local_subscriber.clone()).expect("subscribe"),
      10,
      active,
    )
    .expect("subscribe");
  state
    .apply_command(
      MediatorCommand::try_unsubscribe(local_topic, None, local_subscriber).expect("unsubscribe"),
      11,
      active,
    )
    .expect("unsubscribe");

  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  remote_bucket.put_subscription(remote_topic.clone(), None, remote_subscriber.clone());
  remote_bucket.remove_subscription(remote_topic, None, remote_subscriber, 11);
  state.upsert_remote_bucket(remote_bucket);

  let expired_at = 11 + state.config().removed_entry_ttl().as_millis() as u64;
  let stale_status = TopicRegistryStatus::new(vec![
    (local.clone(), TopicRegistryVersion::new(1)),
    (remote.clone(), TopicRegistryVersion::new(1)),
  ]);
  state.prune_removed_entries(expired_at, &[stale_status]);

  assert!(state.local_bucket().entry(&local_key).is_some());
  let remote_bucket = state.buckets().into_iter().find(|bucket| bucket.owner() == &remote).expect("remote bucket");
  assert!(remote_bucket.entry(&remote_key).is_some());

  let converged_status = TopicRegistryStatus::new(vec![
    (local.clone(), TopicRegistryVersion::new(2)),
    (remote.clone(), TopicRegistryVersion::new(2)),
  ]);
  state.prune_removed_entries(expired_at, &[converged_status]);

  assert!(state.local_bucket().entry(&local_key).is_none());
  let remote_bucket = state.buckets().into_iter().find(|bucket| bucket.owner() == &remote).expect("remote bucket");
  assert!(remote_bucket.entry(&remote_key).is_none());
  assert_eq!(state.local_bucket().version(), TopicRegistryVersion::new(2));
  assert_eq!(remote_bucket.version(), TopicRegistryVersion::new(2));
}
