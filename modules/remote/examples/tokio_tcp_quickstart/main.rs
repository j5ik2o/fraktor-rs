#![allow(clippy::print_stdout)]

#[cfg(not(all(feature = "std", feature = "test-support", feature = "tokio-transport", feature = "tokio-executor")))]
compile_error!("tokio_tcp_quickstart example requires `--features std,test-support,tokio-transport,tokio-executor`");

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    actor_prim::actor_path::{ActorPath, ActorPathParts, GuardianKind},
    config::{ActorSystemConfig, RemotingConfig},
    error::ActorError,
    extension::ExtensionInstallers,
    serialization::SerializationExtensionInstaller,
  },
  std::{
    actor_prim::{Actor, ActorContext},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::StdTickDriverConfig,
    system::ActorSystem,
  },
};
use fraktor_remote_rs::core::{
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, TokioActorRefProviderGeneric,
  TokioActorRefProviderInstaller, TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

const HOST: &str = "127.0.0.1";
const RECEIVER_PORT: u16 = 25530;
const SENDER_PORT: u16 = 25531;
const RECEIVER_GUARDIAN_NAME: &str = "receiver-guardian";
const SENDER_GUARDIAN_NAME: &str = "sender-guardian";

#[tokio::main]
async fn main() -> Result<()> {
  let receiver = build_tokio_tcp_system(
    "tokio-tcp-receiver",
    RECEIVER_PORT,
    Props::from_fn(ReceiverGuardian::new).with_name(RECEIVER_GUARDIAN_NAME),
    receiver_transport_config(),
  )?;
  println!(
    "receiver user guardian path: {}",
    receiver.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );
  let sender = build_tokio_tcp_system(
    "tokio-tcp-sender",
    SENDER_PORT,
    Props::from_fn(SenderGuardian::new).with_name(SENDER_GUARDIAN_NAME),
    sender_transport_config(),
  )?;
  println!(
    "sender user guardian path: {}",
    sender.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  let provider =
    sender.extended().actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>().expect("provider installed");

  provider.watch_remote(receiver_authority_parts()).map_err(|error| anyhow!("{error}"))?;

  let remote_ref = provider.actor_ref(remote_echo_path()).expect("remote actor ref");
  remote_ref.tell(AnyMessage::new("ping over remoting".to_string())).map_err(|error| anyhow!("{error:?}"))?;
  println!("sender -> remote: ping over remoting");

  thread::sleep(Duration::from_millis(200));

  #[cfg(feature = "test-support")]
  {
    if let Some(envelope) = provider.writer_for_test().try_next().map_err(|error| anyhow!("{error:?}"))? {
      println!("writer -> pending envelope: {:?}", envelope);
    } else {
      println!("writer -> empty (delivered via transport)");
    }
  }

  drop(sender);
  drop(receiver);
  Ok(())
}

fn build_tokio_tcp_system(
  system_name: &str,
  canonical_port: u16,
  guardian: Props,
  transport_config: RemotingExtensionConfig,
) -> Result<ActorSystem> {
  let system_config = ActorSystemConfig::<StdToolbox>::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(StdTickDriverConfig::tokio_quickstart())
    // 現状のTokio TCPトランスポートはハンドシェイク層が未実装なため、例ではループバック配送を併用して疎通を確認する
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::from_config(TokioTransportConfig::default()))
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(canonical_port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(transport_config.clone())),
    );
  let system = ActorSystem::new_with_config(&guardian, &system_config).map_err(|error| anyhow!("{error:?}"))?;
  let id = RemotingExtensionId::<StdToolbox>::new(transport_config);
  let _ = system.extended().extension(&id).expect("extension registered");
  Ok(system)
}

fn receiver_transport_config() -> RemotingExtensionConfig {
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp")
}

fn sender_transport_config() -> RemotingExtensionConfig {
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp")
}

fn receiver_authority_parts() -> ActorPathParts {
  ActorPathParts::with_authority("tokio-tcp-receiver", Some((HOST, RECEIVER_PORT)))
}

fn remote_echo_path() -> ActorPath {
  let parts = receiver_authority_parts().with_guardian(GuardianKind::User);
  ActorPath::from_parts(parts).child(RECEIVER_GUARDIAN_NAME).child("echo")
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
