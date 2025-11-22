#![allow(clippy::print_stdout)]

#[cfg(not(all(feature = "std", feature = "test-support", feature = "tokio-transport", feature = "tokio-executor")))]
compile_error!("tokio_tcp_quickstart example requires `--features std,test-support,tokio-transport,tokio-executor`");

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    actor_prim::actor_path::{ActorPath, ActorPathFormatter, ActorPathParts, ActorPathParser, ActorPathScheme, GuardianKind},
    error::ActorError,
    extension::ExtensionInstallers,
    serialization::SerializationExtensionInstaller,
    system::RemotingConfig,
  },
  std::{
    actor_prim::{Actor, ActorContext},
    dispatcher::{DispatchExecutorAdapter, DispatcherConfig, dispatch_executor::TokioExecutor},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_remote_rs::core::{
  default_loopback_setup, RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
  TokioActorRefProviderGeneric, TokioActorRefProviderInstaller, TokioTransportConfig,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

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

  let provider = sender
    .extended()
    .actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>()
    .expect("provider installed");
  provider.watch_remote(receiver_authority_parts()).map_err(|error| anyhow!("{error}"))?;

  let remote_ref = provider.actor_ref(remote_echo_path()).map_err(|error| anyhow!("{error:?}"))?;
  let reply_path = ActorPathFormatter::format(&local_sender_path());
  println!("reply_path canonical: {}", reply_path);

  sender
    .user_guardian_ref()
    .tell(AnyMessage::new(StartPing { target: remote_ref, reply_path, text: "ping over remoting".to_string() }))
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
  // TokioExecutorをデフォルトdispatcherとして設定
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let executor_adapter = DispatchExecutorAdapter::new(ArcShared::new(tokio_executor));
  let default_dispatcher = DispatcherConfig::from_executor(ArcShared::new(executor_adapter));

  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::tokio_quickstart())
    .with_default_dispatcher(default_dispatcher) // デフォルトdispatcherを設定
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
  ActorPathParts::with_authority("tokio-tcp-receiver", Some((HOST, RECEIVER_PORT))).with_scheme(ActorPathScheme::FraktorTcp)
}

fn remote_echo_path() -> ActorPath {
  let parts = receiver_authority_parts().with_guardian(GuardianKind::User);
  ActorPath::from_parts(parts).child(RECEIVER_GUARDIAN_NAME).child("echo")
}

fn sender_authority_parts() -> ActorPathParts {
  ActorPathParts::with_authority("tokio-tcp-sender", Some((HOST, SENDER_PORT))).with_scheme(ActorPathScheme::FraktorTcp)
}

fn local_sender_path() -> ActorPath {
  let parts = sender_authority_parts().with_guardian(GuardianKind::User);
  ActorPath::from_parts(parts).child(SENDER_GUARDIAN_NAME)
}

struct SenderGuardian;

impl SenderGuardian {
  fn new() -> Self {
    Self
  }
}

impl Actor for SenderGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      let payload = format!("{}|{}", cmd.text, cmd.reply_path);
      let envelope = AnyMessage::new(payload).with_reply_to(ctx.self_ref());
      println!("sender: reply_to path = {:?}", envelope.reply_to().and_then(|r| r.path()).map(|p| p.to_string()));
      cmd
        .target
        .tell(envelope)
        .map_err(|e| ActorError::recoverable(format!("send failed: {e:?}")))?;
      println!("sender -> remote: {}", cmd.text);
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
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      if let Some((body, reply_path)) = text.split_once('|') {
        println!("receiver <- {}", body);
        println!("receiver: reply_path = {}", reply_path);
        let reply = format!("pong to: {body}");
        let actor_path = ActorPathParser::parse(reply_path)
          .map_err(|e| ActorError::recoverable(format!("parse: {e:?}")))?;
        let provider = ctx
          .system()
          .extended()
          .actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>()
          .ok_or_else(|| ActorError::recoverable("actor_ref_provider missing".to_string()))?;
        provider
          .actor_ref(actor_path)
          .map_err(|e| ActorError::recoverable(format!("resolve reply_to failed: {e:?}")))?
          .tell(AnyMessage::new(reply))
          .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
      } else {
        println!("receiver <- malformed payload: {text}");
      }
    } else {
      println!("receiver <- unsupported payload");
    }
    Ok(())
  }
}

#[derive(Clone, Debug)]
struct StartPing {
  target: fraktor_actor_rs::std::actor_prim::ActorRef,
  reply_path: String,
  text:   String,
}
