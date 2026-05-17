//! Actor-system level two-node remote delivery proof.

use std::{
  any::{Any, TypeId},
  format,
  net::TcpListener,
  time::Duration,
};

use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Address as ActorAddress, ChildRef, Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::ActorRef,
    deploy::{Deploy, Deployer, RemoteScope, Scope},
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::{DeployableFactoryError, DeployablePropsMetadata, Props},
  },
  event::stream::{
    ClassifierKey, EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, RemotingLifecycleEvent,
    subscriber_handle,
  },
  serialization::{
    SerializationError, SerializationExtensionInstaller, SerializationSetupBuilder, SerializedMessage, Serializer,
    SerializerId,
  },
  system::{ActorSystem, remote::RemotingConfig},
};
use fraktor_remote_adaptor_std_rs::{
  extension_installer::RemotingExtensionInstaller, provider::StdRemoteActorRefProviderInstaller,
  transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  config::RemoteConfig,
};
use fraktor_stream_core_kernel_rs::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source, StreamRefs},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepBoth, KeepRight, StreamDone},
  stage::StageKind,
  stream_ref::{
    STREAM_REF_PROTOCOL_SERIALIZER_NAME, SinkRef, SourceRef, StreamRefProtocolSerializationSetup, StreamRefResolver,
  },
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};
use tokio::{
  sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
  time::{Instant, sleep, timeout},
};

const SYSTEM_NAME: &str = "remote-e2e";
const SOURCE_REF_ENVELOPE_SERIALIZER_NAME: &str = "stream-ref-source-envelope";
const SINK_REF_ENVELOPE_SERIALIZER_NAME: &str = "stream-ref-sink-envelope";
const SOURCE_REF_ENVELOPE_MANIFEST: &str = "fraktor.test.SourceRefEnvelope";
const SINK_REF_ENVELOPE_MANIFEST: &str = "fraktor.test.SinkRefEnvelope";
const SOURCE_REF_ENVELOPE_SERIALIZER_ID: SerializerId = SerializerId::from_raw(710);
const SINK_REF_ENVELOPE_SERIALIZER_ID: SerializerId = SerializerId::from_raw(711);

struct RecordingStringActor {
  tx: UnboundedSender<String>,
}

struct RecordingSourceRefActor {
  tx: UnboundedSender<SourceRef<i32>>,
}

struct RecordingSinkRefActor {
  tx: UnboundedSender<SinkRef<i32>>,
}

struct RecordingDeathwatchActor {
  terminated_tx: UnboundedSender<Pid>,
  ack_tx:        UnboundedSender<&'static str>,
}

struct OrderedDeathwatchActor {
  tx: UnboundedSender<OrderedDeathwatchEvent>,
}

#[derive(Debug, PartialEq, Eq)]
enum OrderedDeathwatchEvent {
  WatchInstalled,
  UserMessage(String),
  Terminated(Pid),
}

struct StartRemoteWatch {
  target: ActorRef,
}

struct StopRemoteWatch {
  target: ActorRef,
}

struct LifecycleRecorder {
  tx: UnboundedSender<RemotingLifecycleEvent>,
}

struct AddressTerminatedRecorder {
  tx: UnboundedSender<EventStreamEvent>,
}

#[derive(Clone)]
struct SerializationSystemSlot {
  system: ArcShared<SpinSyncMutex<Option<ActorSystem>>>,
}

struct SourceRefEnvelope {
  source_ref: ArcShared<SpinSyncMutex<Option<SourceRef<i32>>>>,
}

struct SinkRefEnvelope {
  sink_ref: ArcShared<SpinSyncMutex<Option<SinkRef<i32>>>>,
}

struct SourceRefEnvelopeSerializer {
  system_slot: SerializationSystemSlot,
}

struct SinkRefEnvelopeSerializer {
  system_slot: SerializationSystemSlot,
}

struct OneThenNeverSourceLogic {
  value: Option<i32>,
}

impl LifecycleRecorder {
  fn new(tx: UnboundedSender<RemotingLifecycleEvent>) -> Self {
    Self { tx }
  }
}

impl AddressTerminatedRecorder {
  fn new(tx: UnboundedSender<EventStreamEvent>) -> Self {
    Self { tx }
  }
}

impl EventStreamSubscriber for LifecycleRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::RemotingLifecycle(event) = event {
      let _ = self.tx.send(event.clone());
    }
  }
}

impl EventStreamSubscriber for AddressTerminatedRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.tx.send(event.clone()).expect("address-terminated channel should be open");
  }
}

impl RecordingStringActor {
  fn new(tx: UnboundedSender<String>) -> Self {
    Self { tx }
  }
}

impl RecordingSourceRefActor {
  fn new(tx: UnboundedSender<SourceRef<i32>>) -> Self {
    Self { tx }
  }
}

impl RecordingSinkRefActor {
  fn new(tx: UnboundedSender<SinkRef<i32>>) -> Self {
    Self { tx }
  }
}

impl RecordingDeathwatchActor {
  fn new(terminated_tx: UnboundedSender<Pid>, ack_tx: UnboundedSender<&'static str>) -> Self {
    Self { terminated_tx, ack_tx }
  }
}

impl OrderedDeathwatchActor {
  fn new(tx: UnboundedSender<OrderedDeathwatchEvent>) -> Self {
    Self { tx }
  }
}

impl StartRemoteWatch {
  fn new(target: ActorRef) -> Self {
    Self { target }
  }
}

impl StopRemoteWatch {
  fn new(target: ActorRef) -> Self {
    Self { target }
  }
}

impl SerializationSystemSlot {
  fn new() -> Self {
    Self { system: ArcShared::new(SpinSyncMutex::new(None)) }
  }

  fn set(&self, system: ActorSystem) {
    *self.system.lock() = Some(system);
  }

  fn system(&self) -> Result<ActorSystem, SerializationError> {
    self.system.lock().clone().ok_or(SerializationError::Uninitialized)
  }
}

impl SourceRefEnvelope {
  fn new(source_ref: SourceRef<i32>) -> Self {
    Self { source_ref: ArcShared::new(SpinSyncMutex::new(Some(source_ref))) }
  }

  fn with_source_ref<F>(&self, serialize: F) -> Result<SerializedMessage, SerializationError>
  where
    F: FnOnce(&SourceRef<i32>) -> Result<SerializedMessage, SerializationError>, {
    let guard = self.source_ref.lock();
    let Some(source_ref) = guard.as_ref() else {
      return Err(SerializationError::InvalidFormat);
    };
    serialize(source_ref)
  }

  fn take(&self) -> Option<SourceRef<i32>> {
    self.source_ref.lock().take()
  }
}

impl SinkRefEnvelope {
  fn new(sink_ref: SinkRef<i32>) -> Self {
    Self { sink_ref: ArcShared::new(SpinSyncMutex::new(Some(sink_ref))) }
  }

  fn with_sink_ref<F>(&self, serialize: F) -> Result<SerializedMessage, SerializationError>
  where
    F: FnOnce(&SinkRef<i32>) -> Result<SerializedMessage, SerializationError>, {
    let guard = self.sink_ref.lock();
    let Some(sink_ref) = guard.as_ref() else {
      return Err(SerializationError::InvalidFormat);
    };
    serialize(sink_ref)
  }

  fn take(&self) -> Option<SinkRef<i32>> {
    self.sink_ref.lock().take()
  }
}

impl SourceRefEnvelopeSerializer {
  const fn new(system_slot: SerializationSystemSlot) -> Self {
    Self { system_slot }
  }
}

impl SinkRefEnvelopeSerializer {
  const fn new(system_slot: SerializationSystemSlot) -> Self {
    Self { system_slot }
  }
}

impl OneThenNeverSourceLogic {
  const fn new(value: i32) -> Self {
    Self { value: Some(value) }
  }
}

impl SourceLogic for OneThenNeverSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.value.take() {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }

  fn should_drain_on_shutdown(&self) -> bool {
    false
  }
}

impl Actor for RecordingStringActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      self.tx.send(text.clone()).expect("recording channel should be open");
    }
    Ok(())
  }
}

impl Actor for RecordingSourceRefActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(envelope) = message.downcast_ref::<SourceRefEnvelope>() {
      let source_ref = envelope.take().expect("source ref envelope should contain payload");
      self.tx.send(source_ref).expect("source ref recording channel should be open");
    }
    Ok(())
  }
}

impl Actor for RecordingSinkRefActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(envelope) = message.downcast_ref::<SinkRefEnvelope>() {
      let sink_ref = envelope.take().expect("sink ref envelope should contain payload");
      self.tx.send(sink_ref).expect("sink ref recording channel should be open");
    }
    Ok(())
  }
}

impl Actor for RecordingDeathwatchActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StartRemoteWatch>() {
      context.watch(&command.target).expect("remote watch should install");
      self.ack_tx.send("watch").expect("ack channel should be open");
    } else if let Some(command) = message.downcast_ref::<StopRemoteWatch>() {
      context.unwatch(&command.target).expect("remote unwatch should install");
      self.ack_tx.send("unwatch").expect("ack channel should be open");
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_tx.send(terminated).expect("termination channel should be open");
    Ok(())
  }
}

impl Actor for OrderedDeathwatchActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StartRemoteWatch>() {
      context.watch(&command.target).expect("remote watch should install");
      self.tx.send(OrderedDeathwatchEvent::WatchInstalled).expect("ordered channel should be open");
    } else if let Some(text) = message.downcast_ref::<String>() {
      self.tx.send(OrderedDeathwatchEvent::UserMessage(text.clone())).expect("ordered channel should be open");
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.tx.send(OrderedDeathwatchEvent::Terminated(terminated)).expect("ordered channel should be open");
    Ok(())
  }
}

impl Serializer for SourceRefEnvelopeSerializer {
  fn identifier(&self) -> SerializerId {
    SOURCE_REF_ENVELOPE_SERIALIZER_ID
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let envelope = message.downcast_ref::<SourceRefEnvelope>().ok_or(SerializationError::InvalidFormat)?;
    let system = self.system_slot.system()?;
    let resolver = StreamRefResolver::new(system);
    let nested = envelope.with_source_ref(|source_ref| resolver.source_ref_to_serialized_message(source_ref))?;
    Ok(nested.encode())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let nested = SerializedMessage::decode(bytes)?;
    let system = self.system_slot.system()?;
    let resolver = StreamRefResolver::new(system);
    let source_ref = resolver.resolve_source_ref_message::<i32>(&nested)?;
    Ok(Box::new(SourceRefEnvelope::new(source_ref)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl Serializer for SinkRefEnvelopeSerializer {
  fn identifier(&self) -> SerializerId {
    SINK_REF_ENVELOPE_SERIALIZER_ID
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let envelope = message.downcast_ref::<SinkRefEnvelope>().ok_or(SerializationError::InvalidFormat)?;
    let system = self.system_slot.system()?;
    let resolver = StreamRefResolver::new(system);
    let nested = envelope.with_sink_ref(|sink_ref| resolver.sink_ref_to_serialized_message(sink_ref))?;
    Ok(nested.encode())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let nested = SerializedMessage::decode(bytes)?;
    let system = self.system_slot.system()?;
    let resolver = StreamRefResolver::new(system);
    let sink_ref = resolver.resolve_sink_ref_message::<i32>(&nested)?;
    Ok(Box::new(SinkRefEnvelope::new(sink_ref)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

struct RemoteNode {
  system:    ActorSystem,
  address:   Address,
  installer: ArcShared<RemotingExtensionInstaller>,
}

impl RemoteNode {
  async fn shutdown(self) {
    self.system.terminate().expect("system should terminate");
    timeout(Duration::from_secs(5), self.system.when_terminated())
      .await
      .expect("system should terminate within timeout");
  }
}

fn reserve_port() -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").expect("reserve tcp port");
  listener.local_addr().expect("reserved local addr").port()
}

fn build_node(port: u16, uid: u64) -> RemoteNode {
  build_node_with_remote_config(port, uid, RemoteConfig::new("127.0.0.1"))
}

fn build_node_with_remote_config(port: u16, uid: u64, remote_config: RemoteConfig) -> RemoteNode {
  build_node_with_remote_config_and_deployer(port, uid, remote_config, Deployer::new())
}

fn build_node_with_deployer(port: u16, uid: u64, deployer: Deployer) -> RemoteNode {
  build_node_with_remote_config_and_deployer(port, uid, RemoteConfig::new("127.0.0.1"), deployer)
}

fn build_node_with_remote_config_and_deployer(
  port: u16,
  uid: u64,
  remote_config: RemoteConfig,
  deployer: Deployer,
) -> RemoteNode {
  let address = Address::new(SYSTEM_NAME, "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, remote_config));
  let extension_installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(address.clone(), uid),
    installer.clone(),
  );
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_remoting_config(
      RemotingConfig::default().with_canonical_host(address.host()).with_canonical_port(address.port()),
    )
    .with_deployer(deployer)
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system should build");
  RemoteNode { system, address, installer }
}

fn build_stream_ref_node(port: u16, uid: u64) -> RemoteNode {
  let address = Address::new(SYSTEM_NAME, "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, RemoteConfig::new("127.0.0.1")));
  let system_slot = SerializationSystemSlot::new();
  let serialization_installer = stream_ref_serialization_installer(system_slot.clone());
  let extension_installers = ExtensionInstallers::default()
    .with_extension_installer(serialization_installer)
    .with_shared_extension_installer(installer.clone());
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(address.clone(), uid),
    installer.clone(),
  );
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_remoting_config(
      RemotingConfig::default().with_canonical_host(address.host()).with_canonical_port(address.port()),
    )
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system should build");
  system_slot.set(system.clone());
  RemoteNode { system, address, installer }
}

fn stream_ref_serialization_installer(system_slot: SerializationSystemSlot) -> SerializationExtensionInstaller {
  let source_ref_serializer: ArcShared<dyn Serializer> =
    ArcShared::new(SourceRefEnvelopeSerializer::new(system_slot.clone()));
  let sink_ref_serializer: ArcShared<dyn Serializer> = ArcShared::new(SinkRefEnvelopeSerializer::new(system_slot));
  let setup = SerializationSetupBuilder::new()
    .apply_adapter(&StreamRefProtocolSerializationSetup::new())
    .expect("apply stream ref protocol setup")
    .register_serializer(SOURCE_REF_ENVELOPE_SERIALIZER_NAME, SOURCE_REF_ENVELOPE_SERIALIZER_ID, source_ref_serializer)
    .expect("register source ref envelope serializer")
    .bind::<SourceRefEnvelope>(SOURCE_REF_ENVELOPE_SERIALIZER_NAME)
    .expect("bind source ref envelope")
    .bind_remote_manifest::<SourceRefEnvelope>(SOURCE_REF_ENVELOPE_MANIFEST)
    .expect("bind source ref envelope manifest")
    .register_manifest_route(SOURCE_REF_ENVELOPE_MANIFEST, 0, SOURCE_REF_ENVELOPE_SERIALIZER_NAME)
    .expect("route source ref envelope manifest")
    .register_serializer(SINK_REF_ENVELOPE_SERIALIZER_NAME, SINK_REF_ENVELOPE_SERIALIZER_ID, sink_ref_serializer)
    .expect("register sink ref envelope serializer")
    .bind::<SinkRefEnvelope>(SINK_REF_ENVELOPE_SERIALIZER_NAME)
    .expect("bind sink ref envelope")
    .bind_remote_manifest::<SinkRefEnvelope>(SINK_REF_ENVELOPE_MANIFEST)
    .expect("bind sink ref envelope manifest")
    .register_manifest_route(SINK_REF_ENVELOPE_MANIFEST, 0, SINK_REF_ENVELOPE_SERIALIZER_NAME)
    .expect("route sink ref envelope manifest")
    .set_fallback(STREAM_REF_PROTOCOL_SERIALIZER_NAME)
    .expect("set stream ref protocol fallback")
    .build()
    .expect("build stream ref serialization setup");
  SerializationExtensionInstaller::new(setup)
}

fn spawn_recording_actor(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<String>, ActorPath) {
  let (rx, _child, path) = spawn_recording_child(system, name);
  (rx, path)
}

fn spawn_recording_child(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<String>, ChildRef, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingStringActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("recording actor should spawn");
  let path = child.actor_ref().path().expect("recording actor should have a path");
  (rx, child, path)
}

fn spawn_source_ref_recording_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<SourceRef<i32>>, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingSourceRefActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("source ref recording actor should spawn");
  let path = child.actor_ref().path().expect("source ref recording actor should have a path");
  (rx, path)
}

fn spawn_sink_ref_recording_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<SinkRef<i32>>, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingSinkRefActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("sink ref recording actor should spawn");
  let path = child.actor_ref().path().expect("sink ref recording actor should have a path");
  (rx, path)
}

fn build_stream_materializer(system: ActorSystem) -> ActorMaterializer {
  let config = ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1));
  let mut materializer = ActorMaterializer::new(system, config);
  materializer.start().expect("materializer start");
  materializer
}

fn spawn_deathwatch_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<Pid>, UnboundedReceiver<&'static str>, ActorRef) {
  let (terminated_tx, terminated_rx) = mpsc::unbounded_channel();
  let (ack_tx, ack_rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingDeathwatchActor::new(terminated_tx.clone(), ack_tx.clone()));
  let child = system.actor_of_named(&props, name).expect("death-watch actor should spawn");
  (terminated_rx, ack_rx, child.actor_ref().clone())
}

fn spawn_ordered_deathwatch_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<OrderedDeathwatchEvent>, ActorRef, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || OrderedDeathwatchActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("ordered death-watch actor should spawn");
  let path = child.actor_ref().path().expect("ordered actor should have a path");
  (rx, child.actor_ref().clone(), path)
}

fn subscribe_lifecycle(system: &ActorSystem) -> (UnboundedReceiver<RemotingLifecycleEvent>, EventStreamSubscription) {
  let (tx, rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(LifecycleRecorder::new(tx));
  let subscription = system.event_stream().subscribe(&subscriber);
  (rx, subscription)
}

fn subscribe_address_terminated(
  system: &ActorSystem,
) -> (UnboundedReceiver<EventStreamEvent>, EventStreamSubscription) {
  let (tx, rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(AddressTerminatedRecorder::new(tx));
  let subscription = system.event_stream().subscribe_with_key(ClassifierKey::AddressTerminated, &subscriber);
  (rx, subscription)
}

fn remote_path(address: &Address, local_path: &ActorPath) -> ActorPath {
  ActorPathParser::parse(&format!(
    "fraktor.tcp://{}@{}:{}{}",
    address.system(),
    address.host(),
    address.port(),
    local_path.to_relative_string()
  ))
  .expect("remote actor path should parse")
}

fn remote_deployer_for_child(child_name: &str, address: &Address) -> Deployer {
  remote_deployer_for_path(&format!("/user/{child_name}"), address)
}

fn remote_deployer_for_path(path: &str, address: &Address) -> Deployer {
  let mut deployer = Deployer::new();
  let target = ActorAddress::remote(address.system(), address.host(), address.port());
  deployer.register(path, Deploy::new().with_scope(Scope::Remote(RemoteScope::new(target))));
  deployer
}

async fn recv_until(rx: &mut UnboundedReceiver<String>, expected: String) -> String {
  let deadline = Instant::now() + Duration::from_secs(5);
  let mut seen = Vec::new();
  loop {
    let now = Instant::now();
    if now >= deadline {
      panic!("expected payload receive timeout; expected={expected:?}; seen={seen:?}");
    }
    match timeout(deadline - now, rx.recv()).await {
      | Ok(Some(text)) if text == expected => return text,
      | Ok(Some(text)) => seen.push(text),
      | Ok(None) => panic!("recording channel closed before expected payload; expected={expected:?}; seen={seen:?}"),
      | Err(_) => panic!("expected payload receive timeout; expected={expected:?}; seen={seen:?}"),
    }
  }
}

async fn wait_until_connected(rx: &mut UnboundedReceiver<RemotingLifecycleEvent>, expected_authority: &str) {
  timeout(Duration::from_secs(5), async {
    loop {
      match rx.recv().await {
        | Some(RemotingLifecycleEvent::Connected { authority, .. }) if authority == expected_authority => return,
        | Some(_) => {},
        | None => panic!("lifecycle channel closed before Connected"),
      }
    }
  })
  .await
  .expect("Connected lifecycle event timeout");
}

async fn wait_for_ack(rx: &mut UnboundedReceiver<&'static str>, expected: &'static str) {
  timeout(Duration::from_secs(5), async {
    loop {
      match rx.recv().await {
        | Some(actual) if actual == expected => return,
        | Some(_) => {},
        | None => panic!("watch ack channel closed before {expected}"),
      }
    }
  })
  .await
  .expect("watch ack timeout");
}

async fn wait_for_ordered_event(
  rx: &mut UnboundedReceiver<OrderedDeathwatchEvent>,
  expected: OrderedDeathwatchEvent,
) -> OrderedDeathwatchEvent {
  let deadline = Instant::now() + Duration::from_secs(5);
  let mut seen = Vec::new();
  loop {
    let now = Instant::now();
    if now >= deadline {
      panic!("ordered event receive timeout; expected={expected:?}; seen={seen:?}");
    }
    match timeout(deadline - now, rx.recv()).await {
      | Ok(Some(event)) if event == expected => return event,
      | Ok(Some(event)) => seen.push(event),
      | Ok(None) => panic!("ordered channel closed before expected event; expected={expected:?}; seen={seen:?}"),
      | Err(_) => panic!("ordered event receive timeout; expected={expected:?}; seen={seen:?}"),
    }
  }
}

async fn wait_for_address_terminated(rx: &mut UnboundedReceiver<EventStreamEvent>, expected_authority: &str) {
  timeout(Duration::from_secs(5), async {
    loop {
      match rx.recv().await {
        | Some(EventStreamEvent::AddressTerminated(event)) if event.authority() == expected_authority => return,
        | Some(_) => {},
        | None => panic!("address-terminated channel closed before expected authority {expected_authority}"),
      }
    }
  })
  .await
  .expect("address-terminated event timeout");
}

fn assert_remote_stream_ref_actor_terminated(result: Result<StreamDone, StreamError>) {
  match result {
    | Err(StreamError::RemoteStreamRefActorTerminated { message }) => {
      assert!(message.contains("remote stream ref partner actor terminated"));
    },
    | Err(StreamError::MaterializedResourceRollbackFailed { primary, .. }) => match *primary {
      | StreamError::RemoteStreamRefActorTerminated { message } => {
        assert!(message.contains("remote stream ref partner actor terminated"));
      },
      | error => panic!("expected remote stream ref actor termination as rollback primary, got {error:?}"),
    },
    | other => panic!("expected remote stream ref actor termination failure, got {other:?}"),
  }
}

async fn warm_bidirectional_remote_delivery(
  ref_to_b: &mut ActorRef,
  rx_b: &mut UnboundedReceiver<String>,
  ref_to_a: &mut ActorRef,
  rx_a: &mut UnboundedReceiver<String>,
) {
  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  ref_to_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to node A");
  let _warm_b = recv_until(rx_b, String::from("warm-b")).await;
  let _warm_a = recv_until(rx_a, String::from("warm-a")).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_actor_system_delivery_sends_registered_string_payloads() {
  let node_a = build_node(reserve_port(), 1);
  let node_b = build_node(reserve_port(), 2);
  let (mut lifecycle_a, _subscription_a) = subscribe_lifecycle(&node_a.system);
  let (mut lifecycle_b, _subscription_b) = subscribe_lifecycle(&node_b.system);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a");
  let (mut rx_b, path_b) = spawn_recording_actor(&node_b.system, "receiver-b");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  let mut local_ref_a = node_a
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &path_a))
    .expect("node A should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  local_ref_a.try_tell(AnyMessage::new(String::from("local-a"))).expect("local send to node A");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let _ = recv_until(&mut rx_a, String::from("local-a")).await;

  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B actor through configured provider");
  let mut ref_to_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &path_a))
    .expect("node B should resolve node A actor through configured provider");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  wait_until_connected(&mut lifecycle_a, &node_b.address.to_string()).await;
  wait_until_connected(&mut lifecycle_b, &node_a.address.to_string()).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("to-b"))).expect("send to node B");
  ref_to_a.try_tell(AnyMessage::new(String::from("to-a"))).expect("send to node A");

  let received_b = recv_until(&mut rx_b, String::from("to-b")).await;
  let received_a = recv_until(&mut rx_a, String::from("to-a")).await;
  assert_eq!(received_b, String::from("to-b"));
  assert_eq!(received_a, String::from("to-a"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_node_source_ref_payload_carries_backpressured_elements() {
  let node_a = build_stream_ref_node(reserve_port(), 101);
  let node_b = build_stream_ref_node(reserve_port(), 102);
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "source-ref-warm-a");
  let (mut warm_rx_b, warm_path_b) = spawn_recording_actor(&node_b.system, "source-ref-warm-b");
  let mut warm_ref_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path_b))
    .expect("node A should resolve node B warm actor");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve node A warm actor");
  warm_bidirectional_remote_delivery(&mut warm_ref_b, &mut warm_rx_b, &mut warm_ref_a, &mut warm_rx_a).await;
  let mut materializer_a = build_stream_materializer(node_a.system.clone());
  let mut materializer_b = build_stream_materializer(node_b.system.clone());
  let (mut source_ref_rx, source_ref_receiver_path) =
    spawn_source_ref_recording_actor(&node_b.system, "source-ref-receiver-b");
  let source_ref = Source::from_array([1_i32, 2, 3])
    .into_mat(StreamRefs::source_ref::<i32>(), KeepRight)
    .run(&mut materializer_a)
    .expect("source ref producer should materialize")
    .into_materialized();
  let mut receiver_ref = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &source_ref_receiver_path))
    .expect("node A should resolve node B source-ref receiver");

  receiver_ref
    .try_tell(AnyMessage::new(SourceRefEnvelope::new(source_ref)))
    .expect("source ref envelope should send to node B");

  let received_source_ref = timeout(Duration::from_secs(5), source_ref_rx.recv())
    .await
    .expect("source ref envelope receive timeout")
    .expect("source ref channel should stay open");
  let completion = received_source_ref
    .into_source()
    .run_with(Sink::<i32, _>::collect(), &mut materializer_b)
    .expect("remote source ref consumer should materialize")
    .into_materialized();
  let collected = timeout(Duration::from_secs(5), completion)
    .await
    .expect("source ref stream completion timeout")
    .expect("source ref stream should complete");

  assert_eq!(collected, vec![1_i32, 2, 3]);

  materializer_a.shutdown().expect("node A materializer shutdown");
  materializer_b.shutdown().expect("node B materializer shutdown");
  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_node_source_ref_connection_loss_address_termination_deathwatch_fails_before_protocol_completion() {
  let node_a = build_stream_ref_node(reserve_port(), 121);
  let node_b = build_stream_ref_node(reserve_port(), 122);
  let (mut address_terminated_rx, _address_terminated_subscription) = subscribe_address_terminated(&node_b.system);
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "source-ref-failure-warm-a");
  let (mut warm_rx_b, warm_path_b) = spawn_recording_actor(&node_b.system, "source-ref-failure-warm-b");
  let mut warm_ref_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path_b))
    .expect("node A should resolve node B warm actor");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve node A warm actor");
  warm_bidirectional_remote_delivery(&mut warm_ref_b, &mut warm_rx_b, &mut warm_ref_a, &mut warm_rx_a).await;
  let mut materializer_a = build_stream_materializer(node_a.system.clone());
  let mut materializer_b = build_stream_materializer(node_b.system.clone());
  let (mut source_ref_rx, source_ref_receiver_path) =
    spawn_source_ref_recording_actor(&node_b.system, "source-ref-failure-receiver-b");
  let source_ref = Source::from_logic(StageKind::Custom, OneThenNeverSourceLogic::new(7_i32))
    .into_mat(StreamRefs::source_ref::<i32>(), KeepRight)
    .run(&mut materializer_a)
    .expect("source ref producer should materialize")
    .into_materialized();
  let mut receiver_ref = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &source_ref_receiver_path))
    .expect("node A should resolve node B source-ref receiver");

  receiver_ref
    .try_tell(AnyMessage::new(SourceRefEnvelope::new(source_ref)))
    .expect("source ref envelope should send to node B");

  let received_source_ref = timeout(Duration::from_secs(5), source_ref_rx.recv())
    .await
    .expect("source ref envelope receive timeout")
    .expect("source ref channel should stay open");
  let (element_tx, mut element_rx) = mpsc::unbounded_channel();
  let completion = received_source_ref
    .into_source()
    .run_with(
      Sink::<i32, _>::foreach(move |value| {
        element_tx.send(value).expect("element probe channel should be open");
      }),
      &mut materializer_b,
    )
    .expect("remote source ref consumer should materialize")
    .into_materialized();
  let first = timeout(Duration::from_secs(5), element_rx.recv())
    .await
    .expect("source ref first element timeout")
    .expect("element probe channel should stay open");
  assert_eq!(first, 7_i32);
  sleep(Duration::from_millis(100)).await;

  timeout(Duration::from_secs(5), node_a.installer.shutdown_and_join())
    .await
    .expect("node A remoting shutdown timeout")
    .expect("node A remoting shutdown should succeed");
  warm_ref_a
    .try_tell(AnyMessage::new(String::from("probe-after-node-a-shutdown")))
    .expect("post-shutdown probe should enqueue and surface transport connection loss");
  wait_for_address_terminated(&mut address_terminated_rx, &node_a.address.to_string()).await;
  let result = timeout(Duration::from_secs(5), completion).await.expect("source ref failure completion timeout");
  assert_remote_stream_ref_actor_terminated(result);

  match materializer_a.shutdown() {
    | Ok(()) => {},
    | Err(StreamError::FailedWithContext { message, .. })
      if message.contains("graceful shutdown exceeded drain round limit") => {},
    | Err(error) => panic!("node A materializer shutdown failed unexpectedly: {error:?}"),
  }
  materializer_b.shutdown().expect("node B materializer shutdown");
  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_node_sink_ref_payload_carries_backpressured_elements() {
  let node_a = build_stream_ref_node(reserve_port(), 111);
  let node_b = build_stream_ref_node(reserve_port(), 112);
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "sink-ref-warm-a");
  let (mut warm_rx_b, warm_path_b) = spawn_recording_actor(&node_b.system, "sink-ref-warm-b");
  let mut warm_ref_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path_b))
    .expect("node A should resolve node B warm actor");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve node A warm actor");
  warm_bidirectional_remote_delivery(&mut warm_ref_b, &mut warm_rx_b, &mut warm_ref_a, &mut warm_rx_a).await;
  let mut materializer_a = build_stream_materializer(node_a.system.clone());
  let mut materializer_b = build_stream_materializer(node_b.system.clone());
  let (mut sink_ref_rx, sink_ref_receiver_path) = spawn_sink_ref_recording_actor(&node_a.system, "sink-ref-receiver-a");
  let consumer = StreamRefs::sink_ref::<i32>().into_mat(Sink::<i32, _>::collect(), KeepBoth);
  let (sink_ref, completion) =
    consumer.run(&mut materializer_b).expect("sink ref consumer should materialize").into_materialized();
  let mut receiver_ref = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &sink_ref_receiver_path))
    .expect("node B should resolve node A sink-ref receiver");

  receiver_ref
    .try_tell(AnyMessage::new(SinkRefEnvelope::new(sink_ref)))
    .expect("sink ref envelope should send to node A");

  let received_sink_ref = timeout(Duration::from_secs(5), sink_ref_rx.recv())
    .await
    .expect("sink ref envelope receive timeout")
    .expect("sink ref channel should stay open");
  let _producer = Source::from_array([10_i32, 20, 30])
    .run_with(received_sink_ref.into_sink(), &mut materializer_a)
    .expect("remote sink ref producer should materialize");
  let collected = timeout(Duration::from_secs(5), completion)
    .await
    .expect("sink ref stream completion timeout")
    .expect("sink ref stream should complete");

  assert_eq!(collected, vec![10_i32, 20, 30]);

  materializer_a.shutdown().expect("node A materializer shutdown");
  materializer_b.shutdown().expect("node B materializer shutdown");
  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_deployment_spawns_actor_and_delivers_user_message() {
  let child_name = "deployed-b";
  let node_b = build_node(reserve_port(), 62);
  let node_a = build_node_with_deployer(reserve_port(), 61, remote_deployer_for_child(child_name, &node_b.address));
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "deployment-warm-a");
  let (mut warm_rx, warm_path) = spawn_recording_actor(&node_b.system, "deployment-warm-b");
  let mut warm_ref = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path))
    .expect("node A should resolve warm target on node B");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve warm target on node A");
  warm_bidirectional_remote_delivery(&mut warm_ref, &mut warm_rx, &mut warm_ref_a, &mut warm_rx_a).await;
  let (target_tx, mut target_rx) = mpsc::unbounded_channel();
  node_b.system.extended().register_deployable_actor_factory("recording-string", move |payload: AnyMessage| {
    let payload =
      payload.downcast_ref::<String>().ok_or_else(|| DeployableFactoryError::new("payload must be String"))?;
    if payload != "factory-payload" {
      return Err(DeployableFactoryError::new("unexpected factory payload"));
    }
    let tx = target_tx.clone();
    Ok(Props::from_fn(move || RecordingStringActor::new(tx.clone())))
  });
  let (unused_tx, _unused_rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingStringActor::new(unused_tx.clone())).with_deployable_metadata(
    DeployablePropsMetadata::new("recording-string", AnyMessage::new(String::from("factory-payload"))),
  );
  let system_a = node_a.system.clone();
  let child = tokio::task::spawn_blocking(move || system_a.actor_of_named(&props, child_name))
    .await
    .expect("blocking spawn should complete")
    .expect("remote child should spawn");

  let created_path = child.actor_ref().canonical_path().expect("deployed child canonical path");
  let mut local_created_ref =
    node_b.system.resolve_actor_ref(created_path.clone()).expect("target node should resolve deployed child locally");
  local_created_ref.try_tell(AnyMessage::new(String::from("local-deployed"))).expect("local send to deployed child");
  let local_received = recv_until(&mut target_rx, String::from("local-deployed")).await;
  assert_eq!(local_received, String::from("local-deployed"));

  let mut actor_ref = child.actor_ref().clone();
  actor_ref.try_tell(AnyMessage::new(String::from("deployed-message"))).expect("send to deployed child");

  let received = recv_until(&mut target_rx, String::from("deployed-message")).await;
  assert_eq!(received, String::from("deployed-message"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_node_remote_deployment_parent_observes_child_termination() {
  let parent_name = "deployment-parent-a";
  let child_name = "watched-deployed-b";
  let node_b = build_node(reserve_port(), 72);
  let node_a = build_node_with_deployer(reserve_port(), 71, remote_deployer_for_child(child_name, &node_b.address));
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "deployment-watch-warm-a");
  let (mut warm_rx_b, warm_path_b) = spawn_recording_actor(&node_b.system, "deployment-watch-warm-b");
  let mut warm_ref_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path_b))
    .expect("node A should resolve warm target on node B");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve warm target on node A");
  warm_bidirectional_remote_delivery(&mut warm_ref_b, &mut warm_rx_b, &mut warm_ref_a, &mut warm_rx_a).await;

  let (target_tx, mut target_rx) = mpsc::unbounded_channel();
  node_b.system.extended().register_deployable_actor_factory("watched-recording-string", move |payload: AnyMessage| {
    let payload =
      payload.downcast_ref::<String>().ok_or_else(|| DeployableFactoryError::new("payload must be String"))?;
    if payload != "watched-factory-payload" {
      return Err(DeployableFactoryError::new("unexpected factory payload"));
    }
    let tx = target_tx.clone();
    Ok(Props::from_fn(move || RecordingStringActor::new(tx.clone())))
  });
  let (unused_tx, _unused_rx) = mpsc::unbounded_channel();
  let child_props = Props::from_fn(move || RecordingStringActor::new(unused_tx.clone()))
    .with_name(child_name)
    .with_deployable_metadata(DeployablePropsMetadata::new(
      "watched-recording-string",
      AnyMessage::new(String::from("watched-factory-payload")),
    ));
  let (terminated_tx, mut terminated_rx) = mpsc::unbounded_channel();
  let (ack_tx, mut ack_rx) = mpsc::unbounded_channel();
  let parent_props = Props::from_fn(move || RecordingDeathwatchActor::new(terminated_tx.clone(), ack_tx.clone()));
  let parent = node_a.system.actor_of_named(&parent_props, parent_name).expect("parent actor should spawn");
  let mut parent_ref = parent.actor_ref().clone();
  let system_a = node_a.system.clone();
  let remote_child = tokio::task::spawn_blocking(move || system_a.actor_of_named(&child_props, child_name))
    .await
    .expect("blocking remote child spawn should complete")
    .expect("remote child should spawn");
  let remote_child = remote_child.actor_ref().clone();

  parent_ref
    .try_tell(AnyMessage::new(StartRemoteWatch::new(remote_child.clone())))
    .expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  let mut remote_child_ref = remote_child.clone();
  remote_child_ref
    .try_tell(AnyMessage::new(String::from("after-parent-watch")))
    .expect("barrier send to remote-deployed child");
  let _after_watch = recv_until(&mut target_rx, String::from("after-parent-watch")).await;
  assert!(terminated_rx.try_recv().is_err(), "parent must not observe termination before target child stops");

  let created_path = remote_child.canonical_path().expect("remote child canonical path");
  let local_child =
    node_b.system.resolve_actor_ref(created_path).expect("target node should resolve remote-deployed child locally");
  node_b.system.stop(&local_child).expect("remote-deployed child should stop locally on target node");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote-deployed child termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, remote_child.pid());

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_actor_termination_notifies_watcher() {
  let node_a = build_node(reserve_port(), 11);
  let node_b = build_node(reserve_port(), 12);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-watch");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "watched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "watcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-watch")).await;

  target_b.stop().expect("remote target should stop");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, ref_to_b.pid());

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_shutdown_waits_for_flush_ack_before_run_task_finishes() {
  let node_a = build_node_with_remote_config(
    reserve_port(),
    31,
    RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_secs(2)),
  );
  let node_b = build_node(reserve_port(), 32);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-shutdown-flush");
  let (mut rx_b, path_b) = spawn_recording_actor(&node_b.system, "receiver-b-shutdown-flush");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("before-shutdown"))).expect("send to node B before shutdown");
  tokio::task::yield_now().await;

  timeout(Duration::from_secs(5), node_a.installer.shutdown_and_join())
    .await
    .expect("shutdown flush should complete within timeout")
    .expect("remote shutdown should succeed");

  let received = recv_until(&mut rx_b, String::from("before-shutdown")).await;
  assert_eq!(received, String::from("before-shutdown"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_deathwatch_notification_arrives_after_preceding_remote_message() {
  let node_a = build_node(reserve_port(), 41);
  let node_b = build_node(reserve_port(), 42);
  let (mut ordered_rx, mut ordered_ref, ordered_path) =
    spawn_ordered_deathwatch_actor(&node_a.system, "ordered-watcher-a");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "ordered-watched-b");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_ordered_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &ordered_path))
    .expect("node B should resolve ordered watcher on node A");

  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  ref_to_ordered_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to ordered watcher on node A");
  let _warm_b = recv_until(&mut rx_b, String::from("warm-b")).await;
  let _warm_a =
    wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::UserMessage(String::from("warm-a"))).await;
  ordered_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  let _watch = wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::WatchInstalled).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _after_watch = recv_until(&mut rx_b, String::from("after-watch")).await;

  ref_to_ordered_a
    .try_tell(AnyMessage::new(String::from("before-deathwatch")))
    .expect("send before death-watch notification");
  target_b.stop().expect("remote target should stop");

  let received_message =
    wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::UserMessage(String::from("before-deathwatch")))
      .await;
  let terminated = wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::Terminated(ref_to_b.pid())).await;
  assert_eq!(received_message, OrderedDeathwatchEvent::UserMessage(String::from("before-deathwatch")));
  assert_eq!(terminated, OrderedDeathwatchEvent::Terminated(ref_to_b.pid()));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_zero_flush_timeout_does_not_stall_deathwatch_or_shutdown() {
  let node_a = build_node(reserve_port(), 51);
  let node_b = build_node_with_remote_config(
    reserve_port(),
    52,
    RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::ZERO),
  );
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-timeout");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "timeout-watched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "timeout-watcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _after_watch = recv_until(&mut rx_b, String::from("after-watch")).await;

  target_b.stop().expect("remote target should stop");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, ref_to_b.pid());
  timeout(Duration::from_secs(5), node_b.installer.shutdown_and_join())
    .await
    .expect("zero-timeout shutdown flush should not stall")
    .expect("remote shutdown should succeed");

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_unwatch_suppresses_stale_notification() {
  let node_a = build_node(reserve_port(), 21);
  let node_b = build_node(reserve_port(), 22);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-unwatch");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "unwatched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "unwatcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-watch")).await;
  watcher_ref
    .try_tell(AnyMessage::new(StopRemoteWatch::new(ref_to_b.clone())))
    .expect("unwatch command should enqueue");
  wait_for_ack(&mut ack_rx, "unwatch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-unwatch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-unwatch")).await;

  target_b.stop().expect("remote target should stop");

  assert!(timeout(Duration::from_secs(2), terminated_rx.recv()).await.is_err());

  node_a.shutdown().await;
  node_b.shutdown().await;
}
