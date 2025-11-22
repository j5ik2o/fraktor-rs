#![allow(clippy::print_stdout)]

//! クラスタ Quickstart: 2ノードを同一プロセス内で起動し、リモート経由で ask/reply を往復するサンプル。

use std::{sync::{Arc, Mutex}, thread, time::Duration};

use anyhow::{Result, anyhow, bail};
use fraktor_actor_rs::{
  core::{
    actor_prim::actor_path::{ActorPath, ActorPathFormatter, ActorPathParser, ActorPathParts, GuardianKind},
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
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, TokioActorRefProviderGeneric,
  TokioActorRefProviderInstaller, TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use tokio::sync::oneshot;

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 4050;
const NODE_B_PORT: u16 = 4051;
const RECEIVER_GUARDIAN: &str = "receiver-guardian";
const SENDER_GUARDIAN: &str = "sender-guardian";
const ECHO_NAME: &str = "echo";
const RESPONDER_NAME: &str = "responder";

#[tokio::main]
async fn main() -> Result<()> {
  let seeds = vec![format!("{HOST}:{NODE_A_PORT}"), format!("{HOST}:{NODE_B_PORT}")];
  validate_required(&seeds)?;

  let receiver = build_system(
    "cluster-receiver",
    NODE_A_PORT,
    Props::from_fn(ReceiverGuardian::new).with_name(RECEIVER_GUARDIAN),
  )?;
  let sender = build_system("cluster-sender", NODE_B_PORT, Props::from_fn(SenderGuardian::new).with_name(SENDER_GUARDIAN))?;

  // sender ノードに返信受け取り用 Responder を配置
  let (tx, rx) = oneshot::channel::<String>();
  let shared_tx = Arc::new(Mutex::new(Some(tx)));
  let responder_props = Props::from_fn({
    let shared_tx = shared_tx.clone();
    move || Responder::new(shared_tx.clone())
  })
  .with_name(RESPONDER_NAME);
  sender
    .user_guardian_ref()
    .tell(AnyMessage::new(responder_props))
    .map_err(|e| anyhow!("failed to install responder: {e:?}"))?;

  println!("[info] receiver user guardian: {:?}", receiver.user_guardian_ref().path());
  println!("[info] sender   user guardian: {:?}", sender.user_guardian_ref().path());

  let provider = sender
    .extended()
    .actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>()
    .expect("provider installed");

  provider.watch_remote(receiver_authority_parts()).map_err(|e| anyhow!("watch_remote failed: {e}"))?;

  // 接続確立まで僅かに待機（ハンドシェイクを完了させる）。
  tokio::time::sleep(Duration::from_millis(300)).await;

  let remote_ref = provider.actor_ref(remote_echo_path()).expect("remote actor ref");

  // responder の ActorPath をメッセージに埋め込み、Echo に返信先を教える
  let reply_path = ActorPathFormatter::format(&responder_path());
  let payload = format!("ping from quickstart|{reply_path}");
  remote_ref
    .tell(AnyMessage::new(payload))
    .map_err(|e| anyhow!("tell failed: {e:?}"))?;

  // responder が返信を受け取るのを待機（最大5秒）
  let reply_text = tokio::time::timeout(Duration::from_secs(5), rx)
    .await
    .map_err(|_| anyhow!("timeout waiting reply"))?
    .map_err(|_| anyhow!("responder dropped"))?;

  println!("[info] seeds       : {}", seeds.join(", "));
  println!("[info] remote pid  : {}", ActorPathFormatter::format(&remote_echo_path()));
  println!("[info] reply       : {reply_text}");
  println!("[info] 再実行時はプロセスを停止し、ポート {NODE_A_PORT}/{NODE_B_PORT} の競合を避けてください。");

  // 終了シグナルを送ってクリーンアップ（簡易）
  sender.terminate().ok();
  receiver.terminate().ok();
  thread::sleep(Duration::from_millis(200));
  Ok(())
}

fn validate_required(seeds: &[String]) -> Result<()> {
  let mut missing = Vec::new();
  if seeds.is_empty() {
    missing.push("seeds");
  }
  if missing.is_empty() {
    Ok(())
  } else {
    bail!("必須設定が不足しています: {missing:?}")
  }
}

fn build_system(system_name: &str, port: u16, guardian: Props) -> Result<ActorSystem> {
  // TokioExecutor をデフォルト dispatcher に設定。
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let executor_adapter = DispatchExecutorAdapter::new(ArcShared::new(tokio_executor));
  let default_dispatcher = DispatcherConfig::from_executor(ArcShared::new(executor_adapter));

  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::tokio_quickstart())
    .with_default_dispatcher(default_dispatcher)
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::from_config(TokioTransportConfig::default()))
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"))),
    );

  let system = ActorSystem::new_with_config(&guardian, &system_config).map_err(|e| anyhow!("actor system build failed: {e:?}"))?;

  let id = RemotingExtensionId::<StdToolbox>::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"));
  let _ = system.extended().extension(&id).expect("extension registered");
  Ok(system)
}

fn receiver_authority_parts() -> ActorPathParts {
  ActorPathParts::with_authority("cluster-receiver", Some((HOST, NODE_A_PORT))).with_guardian(GuardianKind::User)
}

fn remote_echo_path() -> ActorPath {
  ActorPath::from_parts(receiver_authority_parts())
    .child(RECEIVER_GUARDIAN)
    .child(ECHO_NAME)
}

fn responder_path() -> ActorPath {
  ActorPath::from_parts(ActorPathParts::with_authority("cluster-sender", Some((HOST, NODE_B_PORT))).with_guardian(GuardianKind::User))
    .child(SENDER_GUARDIAN)
    .child(RESPONDER_NAME)
}

struct SenderGuardian;

impl SenderGuardian {
  fn new() -> Self {
    Self
  }
}

impl Actor for SenderGuardian {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(props) = _message.downcast_ref::<Props>() {
      _ctx.spawn_child(props.as_core())
        .map_err(|e| ActorError::recoverable(format!("spawn responder failed: {e:?}")))?;
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
    // Echo アクターを spawn。
    let props = Props::from_fn(EchoActor::new).with_name(ECHO_NAME);
    ctx
      .spawn_child(&props)
      .map_err(|e| ActorError::recoverable(format!("echo spawn failed: {e:?}")))?;
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
      let mut parts = text.splitn(2, '|');
      let body = parts.next().unwrap_or("");
      let reply_path_str = parts.next().unwrap_or("");
      println!("[info] receiver <- {body}");

      let Ok(reply_path) = ActorPathParser::parse(reply_path_str) else {
        println!("[warn] invalid reply path: {reply_path_str}");
        return Ok(());
      };

      if let Some(provider) = _ctx
        .system()
        .extended()
        .actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>()
      {
        if let Ok(reply_ref) = provider.actor_ref(reply_path) {
          reply_ref
            .tell(AnyMessage::new(format!("echo:{body}")))
            .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
        } else {
          println!("[warn] reply actor not found");
        }
      }
    }
    Ok(())
  }
}

struct Responder {
  tx: Arc<Mutex<Option<oneshot::Sender<String>>>>,
}

impl Responder {
  fn new(tx: Arc<Mutex<Option<oneshot::Sender<String>>>>) -> Self {
    Self { tx }
  }
}

impl Actor for Responder {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      if let Some(tx) = self.tx.lock().unwrap().take() {
        let _ = tx.send(text.clone());
      }
    }
    Ok(())
  }
}
