#![allow(clippy::print_stdout)]

#[cfg(not(all(feature = "std", feature = "test-support", feature = "tokio-transport", feature = "tokio-executor")))]
compile_error!("tokio_tcp_quickstart example requires `--features std,test-support,tokio-transport,tokio-executor`");

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    error::ActorError, extension::ExtensionInstallers, serialization::SerializationExtensionInstaller,
    system::RemotingConfig,
  },
  std::{
    actor_prim::{Actor, ActorContext},
    dispatcher::{DispatcherConfig, dispatch_executor::TokioExecutor},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_remote_rs::core::{
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, TokioActorRefProviderInstaller,
  TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::{
  core::sync::ArcShared,
  std::{StdSyncMutex, runtime_toolbox::StdToolbox},
};

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
    RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"),
  )?;
  println!(
    "receiver user guardian path: {}",
    receiver.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );
  let sender = build_tokio_tcp_system(
    "tokio-tcp-sender",
    SENDER_PORT,
    Props::from_fn(SenderGuardian::new).with_name(SENDER_GUARDIAN_NAME),
    RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"),
  )?;
  println!(
    "sender user guardian path: {}",
    sender.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  sender
    .user_guardian_ref()
    .tell(AnyMessage::new(StartPing {
      // ローカルで取得した ActorRef をそのまま渡すだけ。canonical 付与とリモート配送はランタイムが自動処理する。
      target: receiver.user_guardian_ref(),
      text:   "ping over remoting".to_string(),
    }))
    .map_err(|error| anyhow!("{error:?}"))?;

  thread::sleep(Duration::from_millis(1500));
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
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let default_dispatcher = DispatcherConfig::from_executor(ArcShared::new(StdSyncMutex::new(Box::new(tokio_executor))));

  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::tokio_quickstart())
    .with_default_dispatcher(default_dispatcher)
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

struct SenderGuardian;

impl SenderGuardian {
  fn new() -> Self {
    Self
  }
}

impl Actor for SenderGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      let envelope = AnyMessage::new(Ping { text: cmd.text.clone(), reply_to: ctx.self_ref() });
      println!("sender -> remote: {}", cmd.text);
      cmd.target.tell(envelope).map_err(|e| ActorError::recoverable(format!("send failed: {e:?}")))?;
    } else if let Some(pong) = message.downcast_ref::<String>() {
      println!("sender <- {}", pong);
    }
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
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      ping
        .reply_to
        .tell(AnyMessage::new(ping.text.clone()))
        .map_err(|e| ActorError::recoverable(format!("forward failed: {e:?}")))?;
      println!("receiver <- {}", ping.text);
    }
    Ok(())
  }
}

#[derive(Clone, Debug)]
struct StartPing {
  target: fraktor_actor_rs::std::actor_prim::ActorRef,
  text:   String,
}

#[derive(Clone, Debug)]
struct Ping {
  text:     String,
  reply_to: fraktor_actor_rs::std::actor_prim::ActorRef,
}
