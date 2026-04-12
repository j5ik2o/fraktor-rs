use alloc::{string::String, vec::Vec};
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorContext, ActorLockFactory,
    ActorShared, ActorSharedFactory, Pid, ReceiveTimeoutState, ReceiveTimeoutStateShared,
    ReceiveTimeoutStateSharedFactory,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory, SendOutcome},
    actor_ref_provider::{
      ActorRefProvider, ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory, LocalActorRefProvider,
    },
    context_pipe::{ContextPipeWakerHandle, ContextPipeWakerHandleShared, ContextPipeWakerHandleSharedFactory},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::{
      AnyMessage, AnyMessageView, AskResult,
      message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
    },
    props::Props,
    scheduler::{
      SchedulerConfig, SchedulerShared,
      tick_driver::{
        ManualTestDriver, TickDriverConfig, TickDriverControl, TickDriverControlShared, TickDriverControlSharedFactory,
      },
    },
    setup::ActorSystemConfig,
  },
  dispatch::{
    dispatcher::{
      Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher, MessageDispatcherShared,
      MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
    },
    mailbox::{
      BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared,
      BoundedPriorityMessageQueueStateSharedFactory,
    },
  },
  event::stream::{
    EventStream, EventStreamEvent, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber,
    EventStreamSubscriberShared, EventStreamSubscriberSharedFactory, EventStreamSubscription,
    subscriber_handle_with_shared_factory,
  },
  system::{
    ActorSystem, TerminationSignal,
    shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet, MailboxSharedSetFactory},
  },
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
};
use fraktor_utils_core_rs::core::{
  sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex},
  time::TimerInstant,
};

use crate::core::{
  ClusterApi, ClusterApiError, ClusterEvent, ClusterEventType, ClusterExtension, ClusterExtensionConfig,
  ClusterExtensionInstaller, ClusterProviderError, ClusterRequestError, ClusterResolveError,
  ClusterSubscriptionInitialStateMode, ClusterTopology, MetricsError, TopologyUpdate,
  cluster_provider::{ClusterProvider, NoopClusterProvider},
  downing_provider::DowningProvider,
  grain::{GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainKey},
  identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError, NoopIdentityLookup},
  placement::{ActivatedKind, PlacementDecision, PlacementEvent, PlacementLocality, PlacementResolution},
};

struct CountingSubscriberLockProvider {
  inner: BuiltinSpinSharedFactory,
  event_stream_subscriber_shared: ArcShared<AtomicUsize>,
}

impl CountingSubscriberLockProvider {
  fn new() -> (ArcShared<AtomicUsize>, Self) {
    let event_stream_subscriber_shared = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinSharedFactory::new(),
      event_stream_subscriber_shared: event_stream_subscriber_shared.clone(),
    };
    (event_stream_subscriber_shared, provider)
  }
}

impl ActorLockFactory for CountingSubscriberLockProvider {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    self.inner.create_lock(value)
  }
}

impl MessageDispatcherSharedFactory for CountingSubscriberLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherSharedFactory::create_message_dispatcher_shared(&self.inner, dispatcher)
  }
}

impl ExecutorSharedFactory for CountingSubscriberLockProvider {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    self.inner.create_executor_shared(executor, trampoline)
  }
}

impl ActorRefSenderSharedFactory for CountingSubscriberLockProvider {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&self.inner, sender)
  }
}

impl ActorSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, actor: Box<dyn Actor + Send>) -> ActorShared {
    ActorSharedFactory::create(&self.inner, actor)
  }
}

impl BoundedPriorityMessageQueueStateSharedFactory for CountingSubscriberLockProvider {
  fn create_bounded_priority_message_queue_state_shared(
    &self,
    state: BoundedPriorityMessageQueueState,
  ) -> BoundedPriorityMessageQueueStateShared {
    BoundedPriorityMessageQueueStateSharedFactory::create_bounded_priority_message_queue_state_shared(
      &self.inner,
      state,
    )
  }
}

impl ActorCellStateSharedFactory for CountingSubscriberLockProvider {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    ActorCellStateSharedFactory::create_actor_cell_state_shared(&self.inner, state)
  }
}

impl ReceiveTimeoutStateSharedFactory for CountingSubscriberLockProvider {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateSharedFactory::create_receive_timeout_state_shared(&self.inner, state)
  }
}

impl MessageInvokerSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerSharedFactory::create(&self.inner, invoker)
  }
}

impl SharedMessageQueueFactory for CountingSubscriberLockProvider {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueueFactory::create(&self.inner)
  }
}

impl EventStreamSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamSharedFactory::create(&self.inner, stream)
  }
}

impl EventStreamSubscriberSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    self.event_stream_subscriber_shared.fetch_add(1, Ordering::SeqCst);
    EventStreamSubscriberSharedFactory::create(&self.inner, subscriber)
  }
}

impl MailboxSharedSetFactory for CountingSubscriberLockProvider {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSetFactory::create(&self.inner)
  }
}

impl ActorFutureSharedFactory<AskResult> for CountingSubscriberLockProvider {
  fn create_actor_future_shared(&self, future: ActorFuture<AskResult>) -> ActorFutureShared<AskResult> {
    ActorFutureSharedFactory::create_actor_future_shared(&self.inner, future)
  }
}

impl TickDriverControlSharedFactory for CountingSubscriberLockProvider {
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared {
    TickDriverControlSharedFactory::create_tick_driver_control_shared(&self.inner, control)
  }
}

impl ActorRefProviderHandleSharedFactory<LocalActorRefProvider> for CountingSubscriberLockProvider {
  fn create_actor_ref_provider_handle_shared(
    &self,
    provider: LocalActorRefProvider,
  ) -> ActorRefProviderHandleShared<LocalActorRefProvider> {
    ActorRefProviderHandleSharedFactory::create_actor_ref_provider_handle_shared(&self.inner, provider)
  }
}

impl ContextPipeWakerHandleSharedFactory for CountingSubscriberLockProvider {
  fn create_context_pipe_waker_handle_shared(&self, handle: ContextPipeWakerHandle) -> ContextPipeWakerHandleShared {
    self.inner.create_context_pipe_waker_handle_shared(handle)
  }
}

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  let provider = ArcShared::new(BuiltinSpinSharedFactory::new());
  let lock_provider: ArcShared<
    dyn fraktor_actor_core_rs::core::kernel::event::stream::EventStreamSubscriberSharedFactory,
  > = provider;
  subscriber_handle_with_shared_factory(&lock_provider, subscriber)
}

#[test]
fn external_subscriber_handle_materializes_via_explicit_lock_provider() {
  let (event_stream_subscriber_shared, lock_provider) = CountingSubscriberLockProvider::new();
  let lock_provider: ArcShared<
    dyn fraktor_actor_core_rs::core::kernel::event::stream::EventStreamSubscriberSharedFactory,
  > = ArcShared::new(lock_provider);
  let baseline = event_stream_subscriber_shared.load(Ordering::SeqCst);

  let _subscriber = subscriber_handle_with_shared_factory(&lock_provider, RecordingClusterEvents::new());

  assert_eq!(
    event_stream_subscriber_shared.load(Ordering::SeqCst) - baseline,
    1,
    "external subscribers should materialize via the supplied lock provider"
  );
}

#[test]
fn try_from_system_fails_when_extension_missing() {
  let system = ActorSystem::new_empty();
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

  run_scheduler(&system, Duration::from_millis(1));

  let result = future.with_write(|inner| inner.try_take()).expect("timeout payload");
  assert!(result.is_err(), "expect timeout error");
  let ask_error = result.unwrap_err();
  assert_eq!(ask_error, fraktor_actor_core_rs::core::kernel::actor::messaging::AskError::Timeout);
}

#[test]
fn down_delegates_to_cluster_provider() {
  let downed_provider: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let downed_strategy: ArcShared<SpinSyncMutex<Vec<String>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let downed_for_provider = downed_provider.clone();
  let downed_for_strategy = downed_strategy.clone();

  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingDownProvider { downed: downed_for_provider.clone() })
    })
    .with_downing_provider_factory(move || Box::new(RecordingDowningProvider { downed: downed_for_strategy.clone() }))
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let shared_factory = BuiltinSpinSharedFactory::new();
      let actor_ref_provider_handle_shared =
        shared_factory.create_actor_ref_provider_handle_shared(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
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

  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingMembershipProvider { joined: joined_for_provider.clone(), left: left_for_provider.clone() })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let shared_factory = BuiltinSpinSharedFactory::new();
      let actor_ref_provider_handle_shared =
        shared_factory.create_actor_ref_provider_handle_shared(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
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

  let _ = api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[]);
}

#[test]
fn cluster_api_subscriptions_materialize_filtered_subscribers_via_system_lock_provider() {
  let (event_stream_subscriber_shared, lock_provider) = CountingSubscriberLockProvider::new();
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_shared_factory(lock_provider)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let shared_factory = BuiltinSpinSharedFactory::new();
      let actor_ref_provider_handle_shared =
        shared_factory.create_actor_ref_provider_handle_shared(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder);
  let baseline = event_stream_subscriber_shared.load(Ordering::SeqCst);

  let subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[ClusterEventType::TopologyUpdated]);
  let no_replay = api.subscribe_no_replay(&subscriber, &[ClusterEventType::TopologyUpdated]);

  assert_eq!(
    event_stream_subscriber_shared.load(Ordering::SeqCst) - baseline,
    2,
    "cluster api should materialize both filtered subscribers via the actor-system lock provider"
  );

  api.unsubscribe(subscription.id());
  api.unsubscribe(no_replay.id());
}

fn run_scheduler(system: &ActorSystem, duration: Duration) {
  let scheduler: SchedulerShared = system.state().scheduler();
  let resolution = scheduler.with_read(|inner| inner.config().resolution());
  let resolution_ns = resolution.as_nanos().max(1);
  let ticks = duration.as_nanos().div_ceil(resolution_ns).max(1);
  let now = TimerInstant::from_ticks(ticks as u64, resolution);
  scheduler.with_write(|inner| {
    let _ = inner.run_due(now);
  });
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
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config =
    ClusterExtensionConfig::new().with_advertised_address("node1:8080").with_metrics_enabled(metrics_enabled);
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(identity_lookup_factory);
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let shared_factory = BuiltinSpinSharedFactory::new();
      let actor_ref_provider_handle_shared =
        shared_factory.create_actor_ref_provider_handle_shared(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
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
  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.downed.lock().push(String::from(authority));
    Ok(())
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
