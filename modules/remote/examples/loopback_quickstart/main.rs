#![allow(clippy::print_stdout)]

#[cfg(not(all(feature = "std", feature = "test-support")))]
compile_error!("loopback_quickstart example requires `--features std,test-support`");

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, ActorContext,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  config::{ActorSystemConfig, RemotingConfig},
  error::ActorError,
  extension::ExtensionInstallers,
  messaging::AnyMessageView,
  props::Props,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::SerializationExtensionInstaller,
  system::{ActorSystem, ActorSystemGeneric},
};
use fraktor_remote_rs::core::{
  LoopbackActorRefProvider, LoopbackActorRefProviderInstaller, RemotingExtensionConfig, RemotingExtensionId,
  RemotingExtensionInstaller, default_loopback_setup,
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

const HOST: &str = "127.0.0.1";
const RECEIVER_PORT: u16 = 25520;
const SENDER_PORT: u16 = 25521;

fn main() -> Result<()> {
  let receiver = build_loopback_system(
    "loopback-receiver",
    RECEIVER_PORT,
    Props::from_fn(ReceiverGuardian::new).with_name("receiver-guardian"),
    receiver_transport_config(),
  )?;
  let sender = build_loopback_system(
    "loopback-sender",
    SENDER_PORT,
    Props::from_fn(SenderGuardian::new).with_name("sender-guardian"),
    sender_transport_config(),
  )?;

  let provider = sender.extended().actor_ref_provider::<LoopbackActorRefProvider>().expect("provider installed");

  provider.watch_remote(receiver_authority_parts()).map_err(|error| anyhow!("{error}"))?;

  let remote_ref = provider.actor_ref(remote_echo_path()).expect("remote actor ref");
  remote_ref
    .tell(fraktor_actor_rs::core::messaging::AnyMessage::new("ping over remoting".to_string()))
    .map_err(|error| anyhow!("{error:?}"))?;
  println!("sender -> remote: ping over remoting");

  thread::sleep(Duration::from_millis(100));

  drop(sender);
  drop(receiver);
  Ok(())
}

fn build_loopback_system(
  system_name: &str,
  canonical_port: u16,
  guardian: Props,
  transport_config: RemotingExtensionConfig,
) -> Result<ActorSystem> {
  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(transport_config.clone())),
    )
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(canonical_port));
  let system = ActorSystemGeneric::new_with_config(&guardian, &system_config).map_err(|error| anyhow!("{error:?}"))?;
  let id = RemotingExtensionId::<NoStdToolbox>::new(transport_config);
  let _ = system.extended().extension(&id).expect("extension registered");
  Ok(system)
}

fn receiver_transport_config() -> RemotingExtensionConfig {
  RemotingExtensionConfig::default().with_canonical_host(HOST).with_canonical_port(RECEIVER_PORT)
}

fn sender_transport_config() -> RemotingExtensionConfig {
  RemotingExtensionConfig::default().with_canonical_host(HOST).with_canonical_port(SENDER_PORT)
}

fn receiver_authority_parts() -> ActorPathParts {
  ActorPathParts::with_authority("loopback-receiver", Some((HOST, RECEIVER_PORT)))
}

fn remote_echo_path() -> ActorPath {
  let parts = receiver_authority_parts().with_guardian(GuardianKind::User);
  ActorPath::from_parts(parts).child("echo")
}

struct SenderGuardian;

impl SenderGuardian {
  fn new() -> Self {
    Self
  }
}

impl Actor for SenderGuardian {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct ReceiverGuardian;

impl ReceiverGuardian {
  fn new() -> Self {
    Self
  }
}

impl Actor for ReceiverGuardian {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let props = Props::from_fn(EchoActor::new).with_name("echo");
    ctx
      .spawn_child(&props)
      .map_err(|error| ActorError::recoverable(format!("failed to spawn echo actor: {:?}", error)))?;
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct EchoActor;

impl EchoActor {
  fn new() -> Self {
    Self
  }
}

impl Actor for EchoActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      println!("receiver <- {}", text);
    } else {
      println!("receiver <- unsupported payload");
    }
    Ok(())
  }
}
