//! Remote actor messaging over the network.
//!
//! Demonstrates sending messages between two actor systems through
//! the remoting layer. Uses Tokio TCP transport for real network
//! communication between a sender and a receiver node.
//!
//! Run with:
//! ```bash
//! cargo run -p fraktor-showcases-std --features advanced --example remote_messaging
//! ```

#![allow(clippy::print_stdout)]

use std::time::Duration;

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    actor::{Actor, ActorContext, actor_ref::ActorRef},
    error::ActorError,
    extension::ExtensionInstallers,
    futures::ActorFutureListener,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    serialization::SerializationExtensionInstaller,
    system::{ActorSystemConfig, remote::RemotingConfig},
  },
  std::{
    dispatch::dispatcher::{DispatcherConfig, dispatch_executor::TokioExecutor},
    system::ActorSystem,
  },
};
use fraktor_remote_rs::core::{
  RemotingExtensionInstaller,
  actor_ref_provider::{loopback::default_loopback_setup, tokio::TokioActorRefProviderInstaller},
  remoting_extension::RemotingExtensionConfig,
};
use fraktor_showcases_std::support::tokio_tick_driver_config;

const HOST: &str = "127.0.0.1";
const RECEIVER_PORT: u16 = 25530;
const SENDER_PORT: u16 = 25531;

// --- メッセージ定義 ---

#[derive(Clone, Debug)]
struct StartPing {
  target: ActorRef,
  text:   String,
}

#[derive(Clone, Debug)]
struct Ping {
  text:     String,
  reply_to: ActorRef,
}

// --- 送信側 Guardian アクター ---

struct SenderGuardian;

impl Actor for SenderGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      // Ping メッセージを組み立て、reply_to に自身のアドレスをセット
      let envelope = AnyMessage::new(Ping { text: cmd.text.clone(), reply_to: ctx.self_ref() });
      println!("[sender] -> remote: {}", cmd.text);
      cmd.target.clone().try_tell(envelope).map_err(|e| ActorError::recoverable(format!("send failed: {e:?}")))?;
    } else if let Some(pong) = message.downcast_ref::<String>() {
      // 受信側からの応答を表示
      println!("[sender] <- reply: {pong}");
    }
    Ok(())
  }
}

// --- 受信側 Guardian アクター ---

struct ReceiverGuardian;

impl Actor for ReceiverGuardian {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      println!("[receiver] <- received: {}", ping.text);
      // reply_to アドレスに応答を送信
      let reply = format!("echo:{}", ping.text);
      let mut reply_to = ping.reply_to.clone();
      reply_to
        .try_tell(AnyMessage::new(reply))
        .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
    }
    Ok(())
  }
}

// --- Tokio TCP ベースの ActorSystem 構築 ---

fn build_system(system_name: &str, canonical_port: u16, guardian: Props) -> Result<ActorSystem> {
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let default_dispatcher = DispatcherConfig::from_executor(Box::new(tokio_executor));

  let transport_config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp");
  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(tokio_tick_driver_config())
    .with_default_dispatcher(default_dispatcher.into_core())
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::default())
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(canonical_port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(transport_config)),
    );

  ActorSystem::new_with_config(&guardian, &system_config).map_err(|e| anyhow!("system build failed: {e:?}"))
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("=== Remote Messaging Demo ===");
  println!("2つの ActorSystem 間で TCP 経由のメッセージ送受信を実演\n");

  // 受信側ノードの起動
  let receiver =
    build_system("remote-receiver", RECEIVER_PORT, Props::from_fn(|| ReceiverGuardian).with_name("receiver-guardian"))?;
  println!(
    "receiver guardian path: {}",
    receiver.user_guardian_ref().path().map(|p| p.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  // 送信側ノードの起動
  let sender =
    build_system("remote-sender", SENDER_PORT, Props::from_fn(|| SenderGuardian).with_name("sender-guardian"))?;
  println!(
    "sender guardian path: {}",
    sender.user_guardian_ref().path().map(|p| p.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  // 送信側から受信側へ Ping を送信
  // ActorRef をそのまま渡すだけで、canonical アドレス付与とリモート配送はランタイムが自動処理する
  let mut sender_guardian = sender.user_guardian_ref();
  sender_guardian
    .try_tell(AnyMessage::new(StartPing {
      target: receiver.user_guardian_ref(),
      text:   "hello over remoting".to_string(),
    }))
    .map_err(|e| anyhow!("send StartPing failed: {e:?}"))?;

  // メッセージ処理を待機
  tokio::time::sleep(Duration::from_millis(1500)).await;

  // シャットダウン
  println!("\n--- シャットダウン ---");
  sender.terminate().map_err(|e| anyhow!("terminate sender: {e:?}"))?;
  receiver.terminate().map_err(|e| anyhow!("terminate receiver: {e:?}"))?;
  ActorFutureListener::new(sender.when_terminated()).await;
  ActorFutureListener::new(receiver.when_terminated()).await;

  println!("=== Demo complete ===");
  Ok(())
}
