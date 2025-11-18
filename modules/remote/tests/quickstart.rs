use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  error::ActorError,
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemBuilder, ActorSystemGeneric, AuthorityState},
};
use fraktor_remote_rs::{RemoteActorRefProvider, RemotingControl, RemotingExtensionConfig, RemotingExtensionId};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

struct QuickstartGuardian;

impl Actor for QuickstartGuardian {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system(
  name: &str,
  port: u16,
) -> (
  ActorSystemGeneric<NoStdToolbox>,
  ArcShared<RemoteActorRefProvider<NoStdToolbox>>,
  fraktor_remote_rs::RemotingControlHandle<NoStdToolbox>,
) {
  let props = PropsGeneric::from_fn(|| QuickstartGuardian).with_name(name);
  let remoting_config = RemotingExtensionConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(port)
    .with_auto_start(false);
  let remoting_id = RemotingExtensionId::new(remoting_config.clone());
  let extensions = ExtensionsConfig::default().with_extension_config(remoting_config);
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .with_actor_ref_provider(RemoteActorRefProvider::loopback())
    .build()
    .expect("actor system");
  let extension = system.register_extension(&remoting_id);
  let handle = extension.handle();
  let provider = system.actor_ref_provider::<RemoteActorRefProvider<NoStdToolbox>>().expect("provider registered");
  (system, provider, handle)
}

#[test]
fn quickstart_loopback_provider_flow() {
  let (system_a, provider_a, handle_a) = build_system("system-a", 4100);
  let (_system_b, provider_b, handle_b) = build_system("system-b", 4200);

  handle_a.start().expect("start a");
  handle_b.start().expect("start b");

  let target = ActorPathParts::with_authority("system-b", Some(("127.0.0.1", 4200)));
  provider_a.watch_remote(target.clone()).expect("watch remote");

  let snapshot = provider_a.connections_snapshot();
  assert!(snapshot.iter().any(|entry| entry.authority() == "127.0.0.1:4200"));

  let state = system_a.state().remote_authority_state("127.0.0.1:4200");
  assert!(matches!(state, AuthorityState::Connected));

  assert!(system_a.actor_ref_provider::<RemoteActorRefProvider<NoStdToolbox>>().is_some());

  provider_b
    .watch_remote(ActorPathParts::with_authority("system-a", Some(("127.0.0.1", 4100))))
    .expect("reciprocal watch");
}
