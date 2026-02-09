#![cfg(feature = "test-support")]

extern crate alloc;

use alloc::format;

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::ActorError,
  extension::ExtensionInstallers,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::SerializationExtensionInstaller,
  system::{
    ActorRefProvider, ActorSystemConfigGeneric, ActorSystemGeneric, AuthorityState, RemoteWatchHookShared,
    RemotingConfig,
  },
};
use fraktor_remote_rs::core::{
  LoopbackActorRefProviderGeneric, LoopbackActorRefProviderInstaller, RemotingControl, RemotingControlShared,
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, default_loopback_setup,
};
use fraktor_utils_rs::{core::sync::SharedAccess, std::runtime_toolbox::StdToolbox};

struct NoopActor;

impl Actor<StdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    _message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system(
  config: RemotingExtensionConfig,
) -> (ActorSystemGeneric<StdToolbox>, RemotingControlShared<StdToolbox>) {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("multi-node-guardian");
  let serialization_installer = SerializationExtensionInstaller::new(default_loopback_setup());
  let extensions = ExtensionInstallers::<StdToolbox>::default()
    .with_extension_installer(serialization_installer)
    .with_extension_installer(RemotingExtensionInstaller::new(config.clone()));
  let remoting_config = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(25511);
  let system_config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()))
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_remoting_config(remoting_config);
  let system = ActorSystemGeneric::new_with_config(&props, &system_config).expect("system");
  let id = RemotingExtensionId::new(config);
  let extension = system.extended().extension(&id).expect("extension registered");
  (system, extension.handle())
}

fn remote_path(system_name: &str, host: &str, port: u16, service_name: &str) -> ActorPath {
  let mut parts = ActorPathParts::with_authority(system_name, Some((host, port)));
  parts = parts.with_guardian(GuardianKind::User);
  let mut path = ActorPath::from_parts(parts);
  path = path.child("user");
  path = path.child(service_name);
  path
}

#[tokio::test]
async fn loopback_provider_routes_messages_for_multiple_remote_authorities() -> Result<()> {
  type SharedProvider = RemoteWatchHookShared<StdToolbox, LoopbackActorRefProviderGeneric<StdToolbox>>;
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = build_system(config);

  handle.lock().start().map_err(|error| anyhow!("{error}"))?;
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  let provider = system.extended().actor_ref_provider::<SharedProvider>().expect("provider installed");

  let first_authority = ActorPathParts::with_authority("remote-a", Some(("127.0.0.1", 25520)));
  provider
    .inner()
    .lock()
    .inner()
    .inner()
    .lock()
    .inner_mut()
    .watch_remote(first_authority)
    .map_err(|error| anyhow!("{error}"))?;
  let second_authority = ActorPathParts::with_authority("remote-b", Some(("127.0.0.1", 25521)));
  provider
    .inner()
    .lock()
    .inner()
    .inner()
    .lock()
    .inner_mut()
    .watch_remote(second_authority)
    .map_err(|error| anyhow!("{error}"))?;

  let remote_a = provider.clone().actor_ref(remote_path("remote-a", "127.0.0.1", 25520, "svc-a")).expect("actor ref");
  let remote_b = provider.clone().actor_ref(remote_path("remote-b", "127.0.0.1", 25521, "svc-b")).expect("actor ref");
  remote_a.tell(AnyMessageGeneric::new("to-a".to_string())).expect("send succeeds");
  remote_b.tell(AnyMessageGeneric::new("to-b".to_string())).expect("send succeeds");

  let writer = provider.inner().lock().inner().inner().lock().inner().writer_for_test();
  let first_envelope = writer.with_write(|w| w.try_next()).expect("poll writer").expect("first envelope");
  let second_envelope = writer.with_write(|w| w.try_next()).expect("poll writer").expect("second envelope");
  assert!(writer.with_write(|w| w.try_next()).expect("poll writer").is_none());

  let mut routes = vec![
    format!(
      "{}:{}{}",
      first_envelope.remote_node().host(),
      first_envelope.remote_node().port().expect("port"),
      first_envelope.recipient().to_relative_string()
    ),
    format!(
      "{}:{}{}",
      second_envelope.remote_node().host(),
      second_envelope.remote_node().port().expect("port"),
      second_envelope.recipient().to_relative_string()
    ),
  ];
  routes.sort();
  assert_eq!(routes, vec![
    "127.0.0.1:25520/user/user/svc-a".to_string(),
    "127.0.0.1:25521/user/user/svc-b".to_string(),
  ]);

  let mut snapshots = provider.inner().lock().inner().inner().lock().inner().connections_snapshot();
  snapshots.sort_by(|left, right| left.authority().cmp(right.authority()));
  assert_eq!(snapshots.len(), 2);
  assert_eq!(snapshots[0].authority(), "127.0.0.1:25520");
  assert_eq!(snapshots[1].authority(), "127.0.0.1:25521");
  assert!(snapshots.iter().all(|snapshot| snapshot.state() == &AuthorityState::Unresolved));
  Ok(())
}
