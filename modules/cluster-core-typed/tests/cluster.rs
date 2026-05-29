use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::time::Duration;

extern crate alloc;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    extension::ExtensionInstallers,
    messaging::AnyMessage,
    setup::ActorSystemConfig,
  },
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_actor_core_typed_rs::{TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
    PlacementResolution,
  },
  cluster_provider::ClusterProvider,
  downing_provider::{DowningDecision, DowningInput, DowningProvider, DowningProviderCompatibility},
  extension::{
    ClusterExtension, ClusterExtensionConfig, ClusterExtensionId, ClusterExtensionInstaller, ClusterProviderError,
    ClusterSubscriptionInitialStateMode,
  },
  grain::GrainKey,
  membership::{CurrentClusterState, MembershipVersion, NodeRecord, NodeStatus, NoopGossiper},
  pub_sub::NoopClusterPubSub,
  topology::{BlockListProvider, ClusterEvent, ClusterEventType, ClusterTopology, TopologyUpdate},
};
use fraktor_cluster_core_typed_rs::{
  Cluster, ClusterCommand, ClusterSetup, ClusterStateSubscription, ClusterStateSubscriptionResult, SelfRemoved, SelfUp,
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

#[derive(Debug)]
struct UserMessage;

#[test]
fn cluster_command_delegates_membership_operations_to_kernel_cluster_api() {
  let joined = ArcShared::new(SpinSyncMutex::new(Vec::<String>::new()));
  let left = ArcShared::new(SpinSyncMutex::new(Vec::<String>::new()));
  let downed = ArcShared::new(SpinSyncMutex::new(Vec::<String>::new()));
  let system = typed_system_with_recording_provider(joined.clone(), left.clone(), downed.clone());
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");

  ClusterCommand::Join { address: String::from("node2:8080") }.apply_to(&cluster).expect("join");
  ClusterCommand::JoinSeedNodes { addresses: vec![String::from("node3:8080"), String::from("node4:8080")] }
    .apply_to(&cluster)
    .expect("join seed nodes");
  ClusterCommand::Leave { address: String::from("node2:8080") }.apply_to(&cluster).expect("leave");
  ClusterCommand::Down { address: String::from("node5:8080") }.apply_to(&cluster).expect("down");

  assert_eq!(joined.lock().clone(), vec![
    String::from("node2:8080"),
    String::from("node3:8080"),
    String::from("node4:8080")
  ]);
  assert_eq!(left.lock().clone(), vec![String::from("node2:8080")]);
  assert_eq!(downed.lock().clone(), vec![String::from("node5:8080")]);
}

#[test]
fn cluster_command_join_seed_nodes_returns_first_kernel_error_without_joining_later_seeds() {
  let joined = ArcShared::new(SpinSyncMutex::new(Vec::<String>::new()));
  let system = typed_system_with_failing_join_provider(joined.clone(), "fail:8080");
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");

  let result = (ClusterCommand::JoinSeedNodes {
    addresses: vec![String::from("ok:8080"), String::from("fail:8080"), String::from("after:8080")],
  })
  .apply_to(&cluster);

  assert!(result.is_err());
  assert_eq!(joined.lock().clone(), vec![String::from("ok:8080"), String::from("fail:8080")]);
}

#[test]
fn cluster_current_state_uses_kernel_cluster_state_snapshot() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  extension.on_topology(&topology_update(3, vec![String::from("node2:8080")], Vec::new()));
  let cluster = Cluster::get(&system).expect("cluster");

  let state = cluster.current_state();

  assert_eq!(state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>(), vec![
    "node1:8080",
    "node2:8080"
  ]);
  assert_eq!(state.leader.as_deref(), Some("node1:8080"));
}

#[test]
fn cluster_state_subscription_get_current_state_uses_kernel_snapshot() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  extension.on_topology(&topology_update(5, vec![String::from("node2:8080")], Vec::new()));
  let cluster = Cluster::get(&system).expect("cluster");

  let state = match ClusterStateSubscription::GetCurrentState.apply_to(&cluster).expect("current state") {
    | ClusterStateSubscriptionResult::CurrentState(state) => state,
    | ClusterStateSubscriptionResult::Subscribed(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected current state")
    },
  };

  assert_eq!(state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>(), vec![
    "node1:8080",
    "node2:8080"
  ]);
}

#[test]
fn cluster_state_subscription_subscribe_and_unsubscribe_controls_kernel_subscription() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  extension.on_topology(&topology_update(9, vec![String::from("node2:8080")], Vec::new()));
  let cluster = Cluster::get(&system).expect("cluster");
  let received = ArcShared::new(SpinSyncMutex::new(Vec::<ClusterEvent>::new()));
  let subscriber = typed_cluster_event_ref(received.clone());

  let subscription = match (ClusterStateSubscription::Subscribe {
    subscriber,
    initial_state_mode: ClusterSubscriptionInitialStateMode::AsSnapshot,
    event_types: vec![ClusterEventType::TopologyUpdated],
  })
  .apply_to(&cluster)
  .expect("subscribe")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };

  assert!(subscription.id() > 0);
  assert!(matches!(
    received.lock().first(),
    Some(ClusterEvent::CurrentClusterState { state, .. })
      if state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec![
        "node1:8080",
        "node2:8080",
      ]
  ));

  received.lock().clear();
  ClusterStateSubscription::Unsubscribe { subscription_id: subscription.id() }.apply_to(&cluster).expect("unsubscribe");
  extension.on_topology(&topology_update(10, vec![String::from("node3:8080")], Vec::new()));

  assert!(received.lock().is_empty());
}

#[test]
fn cluster_state_subscription_exposes_delivery_failures() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");
  let subscriber = failing_cluster_event_ref();

  let subscription = match (ClusterStateSubscription::Subscribe {
    subscriber,
    initial_state_mode: ClusterSubscriptionInitialStateMode::AsSnapshot,
    event_types: vec![ClusterEventType::CurrentClusterState],
  })
  .apply_to(&cluster)
  .expect("subscribe")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };

  assert_eq!(subscription.failed_delivery_count(), 1);
}

#[test]
fn cluster_state_subscription_routes_self_up_and_self_removed_events() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");
  let self_up_events = ArcShared::new(SpinSyncMutex::new(Vec::<SelfUp>::new()));
  let self_removed_events = ArcShared::new(SpinSyncMutex::new(Vec::<SelfRemoved>::new()));

  let up_subscription =
    match (ClusterStateSubscription::SubscribeSelfUp { subscriber: typed_self_up_ref(self_up_events.clone()) })
      .apply_to(&cluster)
      .expect("subscribe self up")
    {
      | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
      | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
        panic!("expected subscription")
      },
    };
  let removed_subscription = match (ClusterStateSubscription::SubscribeSelfRemoved {
    subscriber: typed_self_removed_ref(self_removed_events.clone()),
  })
  .apply_to(&cluster)
  .expect("subscribe self removed")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };

  publish_cluster_event(
    system.as_untyped().event_stream(),
    member_status_changed(
      "node1:8080",
      NodeStatus::Joining,
      NodeStatus::Up,
      TimerInstant::from_ticks(21, Duration::from_secs(1)),
    ),
  );
  publish_cluster_event(
    system.as_untyped().event_stream(),
    member_status_changed(
      "node1:8080",
      NodeStatus::Exiting,
      NodeStatus::Removed,
      TimerInstant::from_ticks(22, Duration::from_secs(1)),
    ),
  );

  assert_eq!(self_up_events.lock().len(), 1);
  assert_eq!(self_up_events.lock()[0].authority(), "node1:8080");
  assert_eq!(self_up_events.lock()[0].current_cluster_state().members[0].authority, "node1:8080");
  assert_eq!(self_removed_events.lock().len(), 1);
  assert_eq!(self_removed_events.lock()[0].authority(), "node1:8080");
  assert_eq!(up_subscription.failed_delivery_count(), 0);
  assert_eq!(removed_subscription.failed_delivery_count(), 0);
}

#[test]
fn cluster_state_subscription_records_self_up_delivery_failure() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");

  let subscription = match (ClusterStateSubscription::SubscribeSelfUp { subscriber: failing_self_up_ref() })
    .apply_to(&cluster)
    .expect("subscribe self up")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };
  publish_cluster_event(
    system.as_untyped().event_stream(),
    member_status_changed(
      "node1:8080",
      NodeStatus::Joining,
      NodeStatus::Up,
      TimerInstant::from_ticks(23, Duration::from_secs(1)),
    ),
  );

  assert_eq!(subscription.failed_delivery_count(), 1);
}

#[test]
fn cluster_state_subscription_records_self_removed_delivery_failure() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");

  let subscription = match (ClusterStateSubscription::SubscribeSelfRemoved { subscriber: failing_self_removed_ref() })
    .apply_to(&cluster)
    .expect("subscribe self removed")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };
  publish_cluster_event(
    system.as_untyped().event_stream(),
    member_status_changed(
      "node1:8080",
      NodeStatus::Exiting,
      NodeStatus::Removed,
      TimerInstant::from_ticks(24, Duration::from_secs(1)),
    ),
  );

  assert_eq!(subscription.failed_delivery_count(), 1);
}

#[test]
fn cluster_state_subscription_replies_to_late_self_removed_subscriber_from_seen_removed_state() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");
  publish_cluster_event(
    system.as_untyped().event_stream(),
    member_status_changed(
      "node1:8080",
      NodeStatus::Exiting,
      NodeStatus::Removed,
      TimerInstant::from_ticks(25, Duration::from_secs(1)),
    ),
  );
  let self_removed_events = ArcShared::new(SpinSyncMutex::new(Vec::<SelfRemoved>::new()));

  let subscription = match (ClusterStateSubscription::SubscribeSelfRemoved {
    subscriber: typed_self_removed_ref(self_removed_events.clone()),
  })
  .apply_to(&cluster)
  .expect("subscribe self removed")
  {
    | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
    | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
      panic!("expected subscription")
    },
  };

  assert_eq!(self_removed_events.lock().len(), 1);
  assert_eq!(self_removed_events.lock()[0].previous_status(), NodeStatus::Exiting);
  assert_eq!(subscription.failed_delivery_count(), 0);
}

#[test]
fn cluster_state_subscription_replies_to_late_self_up_subscriber_from_current_state() {
  let system = typed_system_with_recording_provider(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  );
  let extension = installed_cluster_extension(&system);
  extension.start_member().expect("start member");
  let cluster = Cluster::get(&system).expect("cluster");
  let self_up_events = ArcShared::new(SpinSyncMutex::new(Vec::<SelfUp>::new()));

  let subscription =
    match (ClusterStateSubscription::SubscribeSelfUp { subscriber: typed_self_up_ref(self_up_events.clone()) })
      .apply_to(&cluster)
      .expect("subscribe self up")
    {
      | ClusterStateSubscriptionResult::Subscribed(subscription) => subscription,
      | ClusterStateSubscriptionResult::CurrentState(_) | ClusterStateSubscriptionResult::Unsubscribed => {
        panic!("expected subscription")
      },
    };

  assert_eq!(self_up_events.lock().len(), 1);
  assert_eq!(self_up_events.lock()[0].authority(), "node1:8080");
  assert_eq!(self_up_events.lock()[0].current_cluster_state().members[0].status, NodeStatus::Up);
  assert_eq!(subscription.failed_delivery_count(), 0);
}

#[test]
fn cluster_setup_installs_cluster_extension_during_typed_bootstrap() {
  let setup = ClusterSetup::new(cluster_extension_id(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
  ));
  let installers = ExtensionInstallers::default().with_extension_installer(setup);
  let props = TypedProps::<UserMessage>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);

  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  assert!(Cluster::get(&system).is_ok());
  system.terminate().expect("terminate");
}

#[test]
fn self_up_and_self_removed_wrap_self_member_status_events() {
  let observed_at = TimerInstant::from_ticks(7, Duration::from_secs(1));
  let current_state = current_state_with_self_member(NodeStatus::Up);
  let up = SelfUp::new(String::from("node-1"), String::from("node1:8080"), current_state);
  let removed = SelfRemoved::new(String::from("node-1"), String::from("node1:8080"), NodeStatus::Exiting, observed_at);

  assert_eq!(up.node_id(), "node-1");
  assert_eq!(up.authority(), "node1:8080");
  assert_eq!(up.current_cluster_state().members[0].authority, "node1:8080");
  assert_eq!(removed.node_id(), "node-1");
  assert_eq!(removed.authority(), "node1:8080");
  assert_eq!(removed.previous_status(), NodeStatus::Exiting);
  assert_eq!(removed.observed_at(), observed_at);
}

#[test]
fn self_up_derives_from_self_member_up_event() {
  let observed_at = TimerInstant::from_ticks(11, Duration::from_secs(1));
  let event = member_status_changed("node1:8080", NodeStatus::Joining, NodeStatus::Up, observed_at);

  let up = SelfUp::try_from_cluster_event(&event, "node1:8080", current_state_with_self_member(NodeStatus::Up))
    .expect("self up");

  assert_eq!(up.node_id(), "node-node1:8080");
  assert_eq!(up.authority(), "node1:8080");
  assert_eq!(up.current_cluster_state().members[0].status, NodeStatus::Up);
}

#[test]
fn self_up_ignores_other_authority() {
  let observed_at = TimerInstant::from_ticks(12, Duration::from_secs(1));
  let event = member_status_changed("node2:8080", NodeStatus::Joining, NodeStatus::Up, observed_at);

  let up = SelfUp::try_from_cluster_event(&event, "node1:8080", current_state_with_self_member(NodeStatus::Up));

  assert_eq!(up, None);
}

#[test]
fn self_up_ignores_non_up_status() {
  let observed_at = TimerInstant::from_ticks(13, Duration::from_secs(1));
  let event = member_status_changed("node1:8080", NodeStatus::Joining, NodeStatus::Leaving, observed_at);

  let up = SelfUp::try_from_cluster_event(&event, "node1:8080", current_state_with_self_member(NodeStatus::Up));

  assert_eq!(up, None);
}

#[test]
fn self_removed_derives_from_self_member_removed_event() {
  let observed_at = TimerInstant::from_ticks(14, Duration::from_secs(1));
  let event = member_status_changed("node1:8080", NodeStatus::Exiting, NodeStatus::Removed, observed_at);

  let removed = SelfRemoved::try_from_cluster_event(&event, "node1:8080").expect("self removed");

  assert_eq!(removed.node_id(), "node-node1:8080");
  assert_eq!(removed.authority(), "node1:8080");
  assert_eq!(removed.previous_status(), NodeStatus::Exiting);
  assert_eq!(removed.observed_at(), observed_at);
}

#[test]
fn self_removed_ignores_other_authority() {
  let observed_at = TimerInstant::from_ticks(15, Duration::from_secs(1));
  let event = member_status_changed("node2:8080", NodeStatus::Exiting, NodeStatus::Removed, observed_at);

  let removed = SelfRemoved::try_from_cluster_event(&event, "node1:8080");

  assert_eq!(removed, None);
}

#[test]
fn self_removed_ignores_non_removed_status() {
  let observed_at = TimerInstant::from_ticks(16, Duration::from_secs(1));
  let event = member_status_changed("node1:8080", NodeStatus::Up, NodeStatus::Leaving, observed_at);

  let removed = SelfRemoved::try_from_cluster_event(&event, "node1:8080");

  assert_eq!(removed, None);
}

fn member_status_changed(authority: &str, from: NodeStatus, to: NodeStatus, observed_at: TimerInstant) -> ClusterEvent {
  ClusterEvent::MemberStatusChanged {
    node_id: alloc::format!("node-{authority}"),
    authority: authority.to_string(),
    from,
    to,
    observed_at,
  }
}

fn current_state_with_self_member(status: NodeStatus) -> CurrentClusterState {
  CurrentClusterState::new(
    vec![NodeRecord::new(
      String::from("node-node1:8080"),
      String::from("node1:8080"),
      status,
      MembershipVersion::new(1),
      String::from("test"),
      Vec::new(),
    )],
    Vec::new(),
    Vec::new(),
    Some(String::from("node1:8080")),
    BTreeMap::new(),
  )
}

fn typed_system_with_recording_provider(
  joined: ArcShared<SpinSyncMutex<Vec<String>>>,
  left: ArcShared<SpinSyncMutex<Vec<String>>>,
  downed: ArcShared<SpinSyncMutex<Vec<String>>>,
) -> TypedActorSystem<UserMessage> {
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      let joined = joined.clone();
      let left = left.clone();
      let downed = downed.clone();
      move |_event_stream, _block_list, _address| {
        Box::new(RecordingClusterProvider { joined: joined.clone(), left: left.clone(), downed: downed.clone() })
      }
    })
    .with_downing_provider_factory(DowningProviderCompatibility::new("typed-recording-downing-provider"), {
      || Box::new(RecordingDowningProvider { downed: ArcShared::new(SpinSyncMutex::new(Vec::new())) })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let props = TypedProps::<UserMessage>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  TypedActorSystem::create_from_props(&props, config).expect("typed system")
}

fn typed_system_with_failing_join_provider(
  joined: ArcShared<SpinSyncMutex<Vec<String>>>,
  fail_authority: &str,
) -> TypedActorSystem<UserMessage> {
  let fail_authority = String::from(fail_authority);
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      let joined = joined.clone();
      move |_event_stream, _block_list, _address| {
        Box::new(FailingJoinClusterProvider { joined: joined.clone(), fail_authority: fail_authority.clone() })
      }
    })
    .with_downing_provider_factory(DowningProviderCompatibility::new("typed-recording-downing-provider"), {
      || Box::new(RecordingDowningProvider { downed: ArcShared::new(SpinSyncMutex::new(Vec::new())) })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let props = TypedProps::<UserMessage>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  TypedActorSystem::create_from_props(&props, config).expect("typed system")
}

fn installed_cluster_extension(system: &TypedActorSystem<UserMessage>) -> ArcShared<ClusterExtension> {
  system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension")
}

fn cluster_extension_id(
  joined: ArcShared<SpinSyncMutex<Vec<String>>>,
  left: ArcShared<SpinSyncMutex<Vec<String>>>,
  downed: ArcShared<SpinSyncMutex<Vec<String>>>,
) -> ClusterExtensionId {
  ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address("node1:8080"),
    Box::new(RecordingClusterProvider { joined, left, downed }),
    ArcShared::new(EmptyBlockListProvider),
    Box::new(RecordingDowningProvider { downed: ArcShared::new(SpinSyncMutex::new(Vec::new())) }),
    Box::new(NoopGossiper),
    Box::new(NoopClusterPubSub),
    Box::new(StaticIdentityLookup::new("node1:8080")),
  )
}

fn topology_update(version: u64, joined: Vec<String>, left: Vec<String>) -> TopologyUpdate {
  let topology = ClusterTopology::new(version, joined.clone(), left.clone(), Vec::new());
  let mut members = vec![String::from("node1:8080")];
  for authority in &joined {
    if !members.contains(authority) {
      members.push(authority.clone());
    }
  }
  members.retain(|authority| !left.contains(authority));
  TopologyUpdate::new(
    topology,
    members,
    joined,
    left,
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(version, Duration::from_secs(1)),
  )
}

fn typed_cluster_event_ref(received: ArcShared<SpinSyncMutex<Vec<ClusterEvent>>>) -> TypedActorRef<ClusterEvent> {
  TypedActorRef::from_untyped(ActorRef::new(
    Pid::new(100, 0),
    ActorRefSenderShared::new(Box::new(RecordingClusterEventSender { received })),
  ))
}

fn failing_cluster_event_ref() -> TypedActorRef<ClusterEvent> {
  TypedActorRef::from_untyped(ActorRef::new(Pid::new(101, 0), ActorRefSenderShared::new(Box::new(FailingSender))))
}

fn failing_self_up_ref() -> TypedActorRef<SelfUp> {
  TypedActorRef::from_untyped(ActorRef::new(Pid::new(104, 0), ActorRefSenderShared::new(Box::new(FailingSender))))
}

fn failing_self_removed_ref() -> TypedActorRef<SelfRemoved> {
  TypedActorRef::from_untyped(ActorRef::new(Pid::new(105, 0), ActorRefSenderShared::new(Box::new(FailingSender))))
}

fn typed_self_up_ref(received: ArcShared<SpinSyncMutex<Vec<SelfUp>>>) -> TypedActorRef<SelfUp> {
  TypedActorRef::from_untyped(ActorRef::new(
    Pid::new(102, 0),
    ActorRefSenderShared::new(Box::new(RecordingSelfUpSender { received })),
  ))
}

fn typed_self_removed_ref(received: ArcShared<SpinSyncMutex<Vec<SelfRemoved>>>) -> TypedActorRef<SelfRemoved> {
  TypedActorRef::from_untyped(ActorRef::new(
    Pid::new(103, 0),
    ActorRefSenderShared::new(Box::new(RecordingSelfRemovedSender { received })),
  ))
}

fn publish_cluster_event(event_stream: EventStreamShared, event: ClusterEvent) {
  let payload = AnyMessage::new(event);
  let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&extension_event);
}

struct RecordingClusterEventSender {
  received: ArcShared<SpinSyncMutex<Vec<ClusterEvent>>>,
}

impl ActorRefSender for RecordingClusterEventSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(cluster_event) = message.payload().downcast_ref::<ClusterEvent>() {
      self.received.lock().push(cluster_event.clone());
    }
    if let Some(EventStreamEvent::Extension { payload, .. }) = message.payload().downcast_ref::<EventStreamEvent>()
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.received.lock().push(cluster_event.clone());
    }
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

struct RecordingSelfUpSender {
  received: ArcShared<SpinSyncMutex<Vec<SelfUp>>>,
}

impl ActorRefSender for RecordingSelfUpSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(self_up) = message.payload().downcast_ref::<SelfUp>() {
      self.received.lock().push(self_up.clone());
    }
    Ok(SendOutcome::Delivered)
  }
}

struct RecordingSelfRemovedSender {
  received: ArcShared<SpinSyncMutex<Vec<SelfRemoved>>>,
}

impl ActorRefSender for RecordingSelfRemovedSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(self_removed) = message.payload().downcast_ref::<SelfRemoved>() {
      self.received.lock().push(self_removed.clone());
    }
    Ok(SendOutcome::Delivered)
  }
}

struct RecordingClusterProvider {
  joined: ArcShared<SpinSyncMutex<Vec<String>>>,
  left:   ArcShared<SpinSyncMutex<Vec<String>>>,
  downed: ArcShared<SpinSyncMutex<Vec<String>>>,
}

struct FailingJoinClusterProvider {
  joined:         ArcShared<SpinSyncMutex<Vec<String>>>,
  fail_authority: String,
}

impl ClusterProvider for FailingJoinClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn join(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.joined.lock().push(authority.to_string());
    if authority == self.fail_authority {
      return Err(ClusterProviderError::join("join failed"));
    }
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

impl ClusterProvider for RecordingClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.downed.lock().push(authority.to_string());
    Ok(())
  }

  fn join(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.joined.lock().push(authority.to_string());
    Ok(())
  }

  fn leave(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.left.lock().push(authority.to_string());
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct RecordingDowningProvider {
  downed: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl DowningProvider for RecordingDowningProvider {
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    if let DowningInput::ExplicitDown { authority } = input {
      self.downed.lock().push(authority.clone());
    }
    Ok(DowningDecision::Down)
  }
}

struct StaticIdentityLookup {
  authority: String,
}

impl StaticIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string() }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = alloc::format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

struct EmptyBlockListProvider;

impl BlockListProvider for EmptyBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}
