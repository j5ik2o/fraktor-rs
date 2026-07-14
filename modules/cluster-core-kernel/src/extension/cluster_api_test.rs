use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_adaptor_std_rs::{system::create_noop_actor_system, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathScheme, ActorUid},
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::{ExecutionBatch, SchedulerCommand, SchedulerConfig, SchedulerRunnable, SchedulerShared},
    setup::ActorSystemConfig,
  },
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
    subscriber_handle,
  },
  system::{ActorSystem, TerminationSignal},
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess, SpinSyncMutex},
  time::TimerInstant,
};

use crate::{
  ClusterApi, ClusterApiError, ClusterError, ClusterEvent, ClusterEventType, ClusterExtension, ClusterExtensionConfig,
  ClusterProviderError, ClusterRequestError, ClusterResolveError, ClusterSubscriptionInitialStateMode, ClusterTopology,
  MetricsError, TopologyUpdate,
  activation::{
    ActivatedKind, ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError, NoopIdentityLookup,
    PartitionIdentityLookup, PlacementDecision, PlacementEvent, PlacementLocality, PlacementResolution,
  },
  cluster_provider::{ClusterProvider, NoopClusterProvider},
  downing_provider::{DowningDecision, DowningInput, DowningProvider, DowningProviderCompatibility},
  extension::ClusterExtensionInstaller,
  grain::{GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainKey},
  membership::NodeStatus,
  singleton::SingletonStuckPhase,
};

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  subscriber_handle(subscriber)
}

#[test]
fn try_from_system_fails_when_extension_missing() {
  let system = create_noop_actor_system();
  match ClusterApi::try_from_system(&system) {
    | Ok(_) => panic!("extension should be missing"),
    | Err(err) => assert_eq!(err, ClusterApiError::ExtensionNotInstalled),
  }
}

#[test]
fn try_from_system_returns_existing_extension() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let first = ClusterApi::try_from_system(&system).expect("cluster api");
  let second = ClusterApi::try_from_system(&system).expect("cluster api");

  assert!(ArcShared::ptr_eq(&first.extension, &second.extension));
}

#[test]
fn get_fails_when_cluster_not_started() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("not started");
  assert_eq!(err, ClusterResolveError::ClusterNotStarted);
}

#[test]
fn get_fails_when_kind_not_registered() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("kind not registered");
  assert_eq!(err, ClusterResolveError::KindNotRegistered { kind: "user".to_string() });
}

#[test]
fn get_fails_on_invalid_pid_format() {
  let (system, ext) = build_system_with_extension(|| Box::new(InvalidIdentityLookup));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("invalid pid");
  assert!(matches!(err, ClusterResolveError::InvalidPidFormat { .. }));
}

#[test]
fn get_returns_lookup_pending_when_resolution_pending() {
  let (system, ext) = build_system_with_extension(|| Box::new(PendingIdentityLookup::new()));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("pending");
  assert_eq!(err, ClusterResolveError::LookupPending);
}

#[test]
fn get_resolves_actor_ref_for_registered_kind() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let actor_ref = api.get(&identity).expect("resolved actor ref");
  assert_eq!(actor_ref.pid(), Pid::new(1, 0));
}

#[test]
fn remote_path_of_uses_cluster_advertised_authority_for_local_ref() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let local_path = ActorPath::root().child("worker").with_uid(ActorUid::new(42));
  let actor_ref = ActorRef::with_canonical_path(Pid::new(10, 0), TestSender, local_path);

  let remote_path = api.remote_path_of(&actor_ref).expect("remote path");

  assert_eq!(remote_path.to_canonical_uri(), "fraktor.tcp://cellactor@node1:8080/user/worker#42");
  assert_eq!(remote_path.uid().map(|uid| uid.value()), Some(42));
}

#[test]
fn remote_path_of_preserves_existing_remote_authority_and_uid() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let remote_path =
    ActorPathParser::parse("fraktor.tcp://cellactor@node2:8080/user/worker#77").expect("remote path parses");
  let actor_ref = ActorRef::with_canonical_path(Pid::new(11, 0), TestSender, remote_path.clone());

  let resolved = api.remote_path_of(&actor_ref).expect("remote path");

  assert_eq!(resolved.to_canonical_uri(), remote_path.to_canonical_uri());
  assert_eq!(resolved.uid().map(|uid| uid.value()), Some(77));
}

#[test]
fn remote_path_of_reports_invalid_pid_format_when_canonical_path_is_unavailable() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let sender = ActorRefSenderShared::new(Box::new(TestSender));
  let actor_ref = ActorRef::new(Pid::new(12, 0), sender);

  let err = api.remote_path_of(&actor_ref).expect_err("canonical path is required");

  assert!(matches!(err, ClusterResolveError::InvalidPidFormat { .. }));
}

#[test]
fn grain_metrics_returns_disabled_when_metrics_not_enabled() {
  let (_system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  assert_eq!(ext.grain_metrics(), Err(MetricsError::Disabled));
}

#[test]
fn get_publishes_activation_events_and_updates_metrics() {
  let (system, ext) = build_system_with_extension_config(|| Box::new(EventfulIdentityLookup::new("node1:8080")), true);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let event_stream = system.event_stream();
  let (recorder, _subscription) = subscribe_grain_events(&event_stream);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let _ = api.get(&identity).expect("resolved actor ref");

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::ActivationCreated { .. })));
  assert!(events.iter().any(|event| matches!(event, GrainEvent::ActivationPassivated { .. })));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.activations_created(), 1);
  assert_eq!(metrics.activations_passivated(), 1);
}

#[test]
fn scheduler_callback_defers_grain_notifications_until_after_scheduler_write() {
  let (system, ext) = build_system_with_extension(|| Box::new(EventfulIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let scheduler = system.state().scheduler();
  let subscriber = SchedulingGrainSubscriber::new(scheduler.clone());
  let subscriber_handle = test_subscriber_handle(subscriber.clone());
  let _subscription = system.event_stream().subscribe(&subscriber_handle);
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "scheduled").expect("identity");
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(move |_batch: &ExecutionBatch| {
    let _actor_ref = api.get(&identity).expect("resolve from scheduler callback");
  });
  scheduler.with_write(|inner| {
    inner
      .schedule_once(Duration::from_millis(10), SchedulerCommand::RunRunnable { runnable })
      .expect("schedule callback");
  });

  assert_eq!(run_scheduler(&system, Duration::from_millis(10)), 1);

  assert!(subscriber.notified());
}

#[test]
fn configured_idle_passivation_runs_from_install_and_start_to_events_and_metrics() {
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("node1:8080")
    .with_metrics_enabled(true)
    .with_grain_idle_passivation_threshold(Duration::from_secs(2));
  let (system, ext) =
    build_system_with_cluster_config(|| Box::new(PartitionIdentityLookup::with_defaults()), cluster_config);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");
  ext.on_topology(&build_topology_update(1, Vec::new(), Vec::new()));
  let (recorder, _subscription) = subscribe_grain_events(&system.event_stream());
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let idle = ClusterIdentity::new("user", "idle").expect("idle identity");
  let recent = ClusterIdentity::new("user", "recent").expect("recent identity");

  let _ = api.get(&idle).expect("activate idle Grain");
  assert_eq!(run_scheduler(&system, Duration::from_secs(1)), 0);
  let _ = api.get(&recent).expect("activate recent Grain");
  assert_eq!(run_scheduler(&system, Duration::from_secs(1)), 1);

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::ActivationPassivated { key } if *key == idle.key())));
  assert!(
    !events.iter().any(|event| matches!(event, GrainEvent::ActivationPassivated { key } if *key == recent.key()))
  );
  assert_eq!(ext.grain_metrics().expect("metrics").activations_passivated(), 1);
}

#[test]
fn subsecond_access_is_not_passivated_at_the_next_second_boundary() {
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("node1:8080")
    .with_grain_idle_passivation_threshold(Duration::from_secs(1));
  let (system, ext) =
    build_system_with_cluster_config(|| Box::new(PartitionIdentityLookup::with_defaults()), cluster_config);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");
  ext.on_topology(&build_topology_update(1, Vec::new(), Vec::new()));
  let (recorder, _subscription) = subscribe_grain_events(&system.event_stream());
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "recent").expect("recent identity");

  assert_eq!(run_scheduler(&system, Duration::from_millis(990)), 0);
  let _ = api.get(&identity).expect("activate recent Grain");
  assert_eq!(run_scheduler(&system, Duration::from_millis(10)), 1);

  assert!(
    !recorder
      .events()
      .iter()
      .any(|event| matches!(event, GrainEvent::ActivationPassivated { key } if *key == identity.key()))
  );
}

#[test]
fn request_returns_error_when_lookup_fails() {
  let (system, ext) = build_system_with_extension(|| Box::new(NoopIdentityLookup::new()));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  match api.request(&identity, AnyMessage::new(()), None) {
    | Ok(_) => panic!("lookup should fail"),
    | Err(err) => assert_eq!(err, ClusterRequestError::ResolveFailed(ClusterResolveError::LookupFailed)),
  }
}

#[test]
fn request_returns_ok_without_timeout() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let response = api.request(&identity, AnyMessage::new(()), None).expect("request ok");

  assert!(!response.future().with_read(|inner| inner.is_ready()));
}

#[test]
fn request_future_completes_with_timeout_payload() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let future = api.request_future(&identity, AnyMessage::new(()), Some(Duration::from_millis(1))).expect("request");

  assert!(!future.with_read(|inner| inner.is_ready()));

  assert_eq!(run_scheduler(&system, Duration::from_millis(1)), 1);

  let result = future.with_write(|inner| inner.try_take()).expect("timeout payload");
  assert!(result.is_err(), "expect timeout error");
  let ask_error = result.unwrap_err();
  assert_eq!(ask_error, fraktor_actor_core_kernel_rs::actor::messaging::AskError::Timeout);
}

#[test]
fn down_delegates_to_cluster_provider() {
  let downed_provider: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let downed_strategy: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let downed_for_provider = downed_provider.clone();
  let downed_for_strategy = downed_strategy.clone();

  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingDownProvider { downed: downed_for_provider.clone() })
    })
    .with_downing_provider_factory(DowningProviderCompatibility::new("recording-downing-provider"), move || {
      Box::new(RecordingDowningProvider { downed: downed_for_strategy.clone() })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler_config)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let actor_ref_provider_handle_shared =
        ActorRefProviderHandleShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  api.down("node2:8080").expect("down");

  assert_eq!(downed_strategy.lock().clone(), vec![String::from("node2:8080")]);
  assert_eq!(downed_provider.lock().clone(), vec![String::from("node2:8080")]);
}

#[test]
fn join_and_leave_delegate_to_cluster_provider() {
  let joined_provider: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let left_provider: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let joined_for_provider = joined_provider.clone();
  let left_for_provider = left_provider.clone();

  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingMembershipProvider { joined: joined_for_provider.clone(), left: left_for_provider.clone() })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler_config)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let actor_ref_provider_handle_shared =
        ActorRefProviderHandleShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  api.join("node2:8080").expect("join");
  api.leave("node2:8080").expect("leave");

  assert_eq!(joined_provider.lock().clone(), vec![String::from("node2:8080")]);
  assert_eq!(left_provider.lock().clone(), vec![String::from("node2:8080")]);
}

#[test]
fn subscribe_and_unsubscribe_control_event_stream_registration() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[ClusterEventType::TopologyUpdated]);

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);
  assert!(!recorder.events().is_empty());

  recorder.clear();
  api.unsubscribe(subscription.id());

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);
  assert!(recorder.events().is_empty());
}

#[test]
fn subscribe_snapshot_mode_sends_current_cluster_state_first() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);
  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 2
        && state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec![
          "node1:8080",
          "node3:8080",
        ]
        && state.unreachable.is_empty()
        && state.seen_by.is_empty()
        && state.leader.as_deref() == Some("node1:8080")
  ));

  recorder.clear();
  let third = build_topology_update(3, vec![String::from("node4:8080")], Vec::new());
  extension.on_topology(&third);

  let replayed = recorder.events();
  assert_eq!(replayed.len(), 1);
  assert!(matches!(&replayed[0], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 3));
}

#[test]
fn subscribe_snapshot_mode_sends_self_member_before_topology_events() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, .. }
      if state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec!["node1:8080"]
        && state.unreachable.is_empty()
        && state.seen_by.is_empty()
        && state.leader.as_deref() == Some("node1:8080")
  ));
}

#[test]
fn subscribe_snapshot_mode_keeps_current_cluster_state_first_when_topology_updates_after_subscribe() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let events = recorder.events();
  assert_eq!(events.len(), 2);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 1
        && state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec![
          "node1:8080",
          "node2:8080",
        ]
  ));
  assert!(matches!(&events[1], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 2));
}

#[test]
fn current_state_returns_current_cluster_state_snapshot() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(7, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let state = api.current_state();

  assert_eq!(state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>(), vec![
    "node1:8080",
    "node2:8080"
  ]);
  assert!(state.unreachable.is_empty());
  assert!(state.seen_by.is_empty());
  assert_eq!(state.leader.as_deref(), Some("node1:8080"));
}

#[test]
fn prepare_for_full_cluster_shutdown_publishes_preparing_events_once() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");
  extension.on_topology(&build_topology_update(11, vec![String::from("node2:8080")], Vec::new()));

  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription = system.event_stream().subscribe_no_replay(&subscriber);
  let api = ClusterApi::try_from_system(&system).expect("cluster api");

  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown");

  let events = recorder.events();
  assert_eq!(events.len(), 5);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 11
        && state.members.iter().all(|member| member.status == NodeStatus::PreparingForShutdown)
  ));
  assert!(matches!(
    &events[1],
    ClusterEvent::MemberStatusChanged {
      authority,
      from: NodeStatus::Up,
      to: NodeStatus::PreparingForShutdown,
      observed_at,
      ..
    } if authority == "node1:8080" && observed_at.ticks() == 11
  ));
  assert!(matches!(
    &events[2],
    ClusterEvent::MemberPreparingForShutdown { authority, observed_at, .. }
      if authority == "node1:8080" && observed_at.ticks() == 11
  ));
  assert!(matches!(
    &events[3],
    ClusterEvent::MemberStatusChanged {
      authority,
      from: NodeStatus::Up,
      to: NodeStatus::PreparingForShutdown,
      observed_at,
      ..
    } if authority == "node2:8080" && observed_at.ticks() == 11
  ));
  assert!(matches!(
    &events[4],
    ClusterEvent::MemberPreparingForShutdown { authority, observed_at, .. }
      if authority == "node2:8080" && observed_at.ticks() == 11
  ));
  let state = api.current_state();
  assert!(state.members.iter().all(|member| member.status == NodeStatus::PreparingForShutdown));

  recorder.clear();
  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown is idempotent");

  assert!(recorder.events().is_empty());
}

#[test]
fn prepare_for_full_cluster_shutdown_notifies_current_state_subscribers() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");
  extension.on_topology(&build_topology_update(11, vec![String::from("node2:8080")], Vec::new()));

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription = api.subscribe_no_replay(&subscriber, &[ClusterEventType::CurrentClusterState]);

  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown");

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 11
        && state.members.iter().all(|member| member.status == NodeStatus::PreparingForShutdown)
  ));
}

#[test]
fn prepare_for_full_cluster_shutdown_notifies_new_members_on_rerun() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");
  extension.on_topology(&build_topology_update(11, vec![String::from("node2:8080")], Vec::new()));

  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription = system.event_stream().subscribe_no_replay(&subscriber);
  let api = ClusterApi::try_from_system(&system).expect("cluster api");

  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown");
  recorder.clear();
  let update = TopologyUpdate::new(
    ClusterTopology::new(12, vec![String::from("node3:8080")], Vec::new(), Vec::new()),
    vec![String::from("node1:8080"), String::from("node2:8080"), String::from("node3:8080")],
    vec![String::from("node3:8080")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(12, Duration::from_secs(1)),
  );
  extension.on_topology(&update);
  recorder.clear();

  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown again");

  let events = recorder.events();
  assert_eq!(events.len(), 3);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 12
        && state.members.iter().all(|member| member.status == NodeStatus::PreparingForShutdown)
  ));
  assert!(matches!(
    &events[1],
    ClusterEvent::MemberStatusChanged {
      authority,
      from: NodeStatus::Up,
      to: NodeStatus::PreparingForShutdown,
      observed_at,
      ..
    } if authority == "node3:8080" && observed_at.ticks() == 12
  ));
  assert!(matches!(
    &events[2],
    ClusterEvent::MemberPreparingForShutdown { authority, observed_at, .. }
      if authority == "node3:8080" && observed_at.ticks() == 12
  ));
}

#[test]
fn prepare_for_full_cluster_shutdown_allows_subscribers_to_read_current_state() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");
  extension.on_topology(&build_topology_update(11, vec![String::from("node2:8080")], Vec::new()));

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let state_reader = CurrentStateDuringShutdownSubscriber::new(api.clone());
  let subscriber = test_subscriber_handle(state_reader.clone());
  let _subscription = system.event_stream().subscribe_no_replay(&subscriber);

  api.prepare_for_full_cluster_shutdown().expect("prepare shutdown");

  let statuses = state_reader.statuses();
  assert_eq!(statuses.len(), 2);
  assert!(statuses.iter().all(|snapshot| snapshot.iter().all(|status| *status == NodeStatus::PreparingForShutdown)));
}

#[test]
fn prepare_for_full_cluster_shutdown_rejects_client_mode() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_client().expect("start client");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let err = api.prepare_for_full_cluster_shutdown().expect_err("client mode rejected");

  assert!(matches!(
    err,
    ClusterError::Provider(ClusterProviderError::ShutdownFailed(reason))
      if reason == "full-cluster shutdown preparation requires member mode"
  ));
}

#[test]
fn subscribe_no_replay_skips_buffered_cluster_events() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription = api.subscribe_no_replay(&subscriber, &[ClusterEventType::TopologyUpdated]);
  assert!(recorder.events().is_empty());

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 2));
}

#[test]
#[should_panic(expected = "at least one cluster event type is required")]
fn subscribe_panics_when_event_type_filter_is_empty() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder);

  drop(api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[]));
}

#[test]
fn subscribe_with_shutdown_event_types_receives_only_shutdown_events() {
  // 購読フィルタに MemberPreparingForShutdown / MemberReadyForShutdown のみを指定した購読者が
  // MemberStatusChanged は受信せず、shutdown 進行イベントのみを受信することを検証する（要件 2.4）
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let _subscription = api.subscribe_no_replay(&subscriber, &[
    ClusterEventType::MemberPreparingForShutdown,
    ClusterEventType::MemberReadyForShutdown,
  ]);

  // MemberStatusChanged を publish（フィルタ外なので受信しないはず）
  let event_stream = system.event_stream();
  let status_changed = ClusterEvent::MemberStatusChanged {
    node_id:     String::from("node2"),
    authority:   String::from("node2:8080"),
    from:        NodeStatus::Joining,
    to:          NodeStatus::Up,
    observed_at: TimerInstant::from_ticks(1, Duration::from_secs(1)),
  };
  event_stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(status_changed),
  });

  // MemberPreparingForShutdown を publish（フィルタ内なので受信するはず）
  let preparing = ClusterEvent::MemberPreparingForShutdown {
    node_id:     String::from("node2"),
    authority:   String::from("node2:8080"),
    observed_at: TimerInstant::from_ticks(2, Duration::from_secs(1)),
  };
  event_stream
    .publish(&EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(preparing) });

  // MemberReadyForShutdown を publish（フィルタ内なので受信するはず）
  let ready = ClusterEvent::MemberReadyForShutdown {
    node_id:     String::from("node2"),
    authority:   String::from("node2:8080"),
    observed_at: TimerInstant::from_ticks(3, Duration::from_secs(1)),
  };
  event_stream
    .publish(&EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(ready) });

  let events = recorder.events();
  // MemberStatusChanged は受信していない
  assert!(
    !events.iter().any(|e| matches!(e, ClusterEvent::MemberStatusChanged { .. })),
    "MemberStatusChanged はフィルタ外なので受信してはならない: {events:?}"
  );
  // shutdown 進行イベントは 2 件受信している
  assert_eq!(events.len(), 2, "受信イベント数が 2 件であること: {events:?}");
  assert!(
    matches!(&events[0], ClusterEvent::MemberPreparingForShutdown { authority, .. } if authority == "node2:8080"),
    "1 件目は MemberPreparingForShutdown であること: {:?}",
    events[0]
  );
  assert!(
    matches!(&events[1], ClusterEvent::MemberReadyForShutdown { authority, .. } if authority == "node2:8080"),
    "2 件目は MemberReadyForShutdown であること: {:?}",
    events[1]
  );
}

#[test]
fn subscribe_with_singleton_stuck_filter_receives_only_stuck_events() {
  // SingletonHandOverStuck フィルタの購読者だけが stuck 通知を受信し、
  // 他種別フィルタ（MemberStatusChanged）の購読者は受信しないことを検証する（要件 7.3）
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");

  // SingletonHandOverStuck フィルタで購読する購読者
  let stuck_recorder = RecordingClusterEvents::new();
  let stuck_subscriber = test_subscriber_handle(stuck_recorder.clone());
  let _stuck_subscription = api.subscribe_no_replay(&stuck_subscriber, &[ClusterEventType::SingletonHandOverStuck]);

  // 他種別フィルタ（MemberStatusChanged）で購読する購読者
  let other_recorder = RecordingClusterEvents::new();
  let other_subscriber = test_subscriber_handle(other_recorder.clone());
  let _other_subscription = api.subscribe_no_replay(&other_subscriber, &[ClusterEventType::MemberStatusChanged]);

  // SingletonHandOverStuck を EventStream 経由でテスト発行する
  let event_stream = system.event_stream();
  let stuck_event = ClusterEvent::SingletonHandOverStuck {
    singleton_name: String::from("my-singleton"),
    phase:          SingletonStuckPhase::HandingOver,
    observed_at:    TimerInstant::from_ticks(1, Duration::from_secs(1)),
  };
  event_stream
    .publish(&EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(stuck_event) });

  // SingletonHandOverStuck フィルタの購読者は stuck 通知を 1 件受信する
  let stuck_events = stuck_recorder.events();
  assert_eq!(stuck_events.len(), 1, "SingletonHandOverStuck フィルタ購読者は 1 件受信すること: {stuck_events:?}");
  assert!(
    matches!(
      &stuck_events[0],
      ClusterEvent::SingletonHandOverStuck { singleton_name, .. } if singleton_name == "my-singleton"
    ),
    "受信イベントは SingletonHandOverStuck であること: {:?}",
    stuck_events[0]
  );

  // MemberStatusChanged フィルタの購読者は stuck 通知を受信しない
  let other_events = other_recorder.events();
  assert!(
    other_events.is_empty(),
    "MemberStatusChanged フィルタ購読者は SingletonHandOverStuck を受信してはならない: {other_events:?}"
  );
}

fn run_scheduler(system: &ActorSystem, duration: Duration) -> usize {
  let scheduler: SchedulerShared = system.state().scheduler();
  let (current_tick, resolution) = scheduler.with_read(|inner| (inner.current_tick(), inner.config().resolution()));
  let resolution_ns = resolution.as_nanos().max(1);
  let ticks = duration.as_nanos().div_ceil(resolution_ns).max(1);
  let now = TimerInstant::from_ticks(current_tick.saturating_add(ticks as u64), resolution);
  scheduler.with_write(|inner| inner.run_due(now))
}

fn build_system_with_extension<F>(identity_lookup_factory: F) -> (ActorSystem, ArcShared<ClusterExtension>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  build_system_with_extension_config(identity_lookup_factory, false)
}

fn build_system_with_extension_config<F>(
  identity_lookup_factory: F,
  metrics_enabled: bool,
) -> (ActorSystem, ArcShared<ClusterExtension>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  let cluster_config =
    ClusterExtensionConfig::new().with_advertised_address("node1:8080").with_metrics_enabled(metrics_enabled);
  build_system_with_cluster_config(identity_lookup_factory, cluster_config)
}

fn build_system_with_cluster_config<F>(
  identity_lookup_factory: F,
  cluster_config: ClusterExtensionConfig,
) -> (ActorSystem, ArcShared<ClusterExtension>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(identity_lookup_factory);
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler_config)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let actor_ref_provider_handle_shared =
        ActorRefProviderHandleShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (system, extension)
}

#[derive(Clone)]
struct RecordingGrainEvents {
  events: ArcShared<SpinSyncMutex<Vec<GrainEvent>>>,
}

impl RecordingGrainEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<GrainEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingGrainEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == GRAIN_EVENT_STREAM_NAME
      && let Some(grain_event) = payload.payload().downcast_ref::<GrainEvent>()
    {
      self.events.lock().push(grain_event.clone());
    }
  }
}

fn subscribe_grain_events(event_stream: &EventStreamShared) -> (RecordingGrainEvents, EventStreamSubscription) {
  let recorder = RecordingGrainEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (recorder, subscription)
}

#[derive(Clone)]
struct SchedulingGrainSubscriber {
  scheduler: SchedulerShared,
  notified:  ArcShared<SpinSyncMutex<bool>>,
}

impl SchedulingGrainSubscriber {
  fn new(scheduler: SchedulerShared) -> Self {
    Self { scheduler, notified: ArcShared::new(SpinSyncMutex::new(false)) }
  }

  fn notified(&self) -> bool {
    *self.notified.lock()
  }
}

impl EventStreamSubscriber for SchedulingGrainSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == GRAIN_EVENT_STREAM_NAME
      && matches!(payload.payload().downcast_ref::<GrainEvent>(), Some(GrainEvent::ActivationCreated { .. }))
    {
      self.scheduler.with_write(|inner| {
        inner.schedule_once(Duration::from_millis(10), SchedulerCommand::Noop).expect("schedule from Grain subscriber");
      });
      *self.notified.lock() = true;
    }
  }
}

#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<SpinSyncMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
  }

  fn clear(&self) {
    self.events.lock().clear();
  }
}

impl EventStreamSubscriber for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.lock().push(cluster_event.clone());
    }
  }
}

#[derive(Clone)]
struct CurrentStateDuringShutdownSubscriber {
  api:      ClusterApi,
  statuses: ArcShared<SpinSyncMutex<Vec<Vec<NodeStatus>>>>,
}

impl CurrentStateDuringShutdownSubscriber {
  fn new(api: ClusterApi) -> Self {
    Self { api, statuses: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn statuses(&self) -> Vec<Vec<NodeStatus>> {
    self.statuses.lock().clone()
  }
}

impl EventStreamSubscriber for CurrentStateDuringShutdownSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(ClusterEvent::MemberPreparingForShutdown { .. }) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      let statuses = self.api.current_state().members.into_iter().map(|member| member.status).collect();
      self.statuses.lock().push(statuses);
    }
  }
}

fn build_topology_update(version: u64, joined: Vec<String>, left: Vec<String>) -> TopologyUpdate {
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

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
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
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

struct EventfulIdentityLookup {
  authority: String,
  events:    Vec<PlacementEvent>,
}

impl EventfulIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string(), events: Vec::new() }
  }
}

impl IdentityLookup for EventfulIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = format!("{}::{}", self.authority, key.value());
    self.events.push(PlacementEvent::Activated {
      key:         key.clone(),
      pid:         pid.clone(),
      observed_at: now,
    });
    self.events.push(PlacementEvent::Passivated { key: key.clone(), observed_at: now });
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }

  fn drain_events(&mut self) -> Vec<PlacementEvent> {
    core::mem::take(&mut self.events)
  }
}

struct InvalidIdentityLookup;

impl IdentityLookup for InvalidIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: "invalid".to_string(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid:      "invalid_pid".to_string(),
    })
  }
}

struct PendingIdentityLookup;

impl PendingIdentityLookup {
  fn new() -> Self {
    Self
  }
}

impl IdentityLookup for PendingIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, _key: &GrainKey, _now: u64) -> Result<PlacementResolution, LookupError> {
    Err(LookupError::Pending)
  }
}

struct RecordingDownProvider {
  downed: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl ClusterProvider for RecordingDownProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.downed.lock().push(String::from(authority));
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct RecordingMembershipProvider {
  joined: ArcShared<SpinSyncMutex<Vec<String>>>,
  left:   ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl ClusterProvider for RecordingMembershipProvider {
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
    self.joined.lock().push(String::from(authority));
    Ok(())
  }

  fn leave(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.left.lock().push(String::from(authority));
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

struct TestActorRefProvider {
  system: ActorSystem,
}

impl TestActorRefProvider {
  fn new(system: ActorSystem) -> Self {
    Self { system }
  }
}

impl ActorRefProvider for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Ok(ActorRef::with_system(Pid::new(1, 0), TestSender, &self.system.state()))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}
