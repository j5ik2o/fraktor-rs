#![allow(clippy::print_stdout)]

//! Cluster quickstart (cluster-capable sample)
//! - Two nodes (4050/4051) using Membership/Gossip/VirtualActorRegistry
//! - Select an owner via Rendezvous and spawn keyed actors via VirtualActorRegistry
//! - Reply routing is confirmed via in-band ActorRef on message payloads

use std::{
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

use anyhow::{Result, anyhow, bail};
use fraktor_actor_rs::{
  core::{
    error::ActorError, extension::ExtensionInstallers, serialization::SerializationExtensionInstaller,
    system::RemotingConfig,
  },
  std::{
    actor_prim::{Actor, ActorContext, ActorRef},
    dispatcher::{DispatchExecutorAdapter, DispatcherConfig, dispatch_executor::TokioExecutor},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_cluster_rs::core::{GrainKey, MembershipDelta, MembershipTable, RendezvousHasher, VirtualActorRegistry};
use fraktor_remote_rs::core::{
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, TokioActorRefProviderInstaller,
  TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use tokio::sync::oneshot;

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 4050;
const NODE_B_PORT: u16 = 4051;
const SYSTEM_A: &str = "cluster-receiver";
const SYSTEM_B: &str = "cluster-sender";
const HUB_NAME: &str = "grains";
const SAMPLE_KEY: &str = "user:va-1";

#[tokio::main]
async fn main() -> Result<()> {
  let seeds = vec![format!("{HOST}:{NODE_A_PORT}"), format!("{HOST}:{NODE_B_PORT}")];
  validate_required(&seeds)?;

  // 返信受け取り用 channel
  let (tx, rx) = oneshot::channel::<String>();
  let tx_shared = Arc::new(Mutex::new(Some(tx)));

  // Membership を事前に揃え、IdentityTable へ配布するための delta を作成
  let mut membership_a = MembershipTable::new(3);
  let delta_a = membership_a.try_join("node-a".to_string(), format!("{HOST}:{NODE_A_PORT}")).expect("join a");
  let delta_b = membership_a.try_join("node-b".to_string(), format!("{HOST}:{NODE_B_PORT}")).expect("join b");
  let full_delta =
    MembershipDelta::new(delta_a.from, delta_b.to, vec![delta_a.entries[0].clone(), delta_b.entries[0].clone()]);

  // ノードA（owner 側になる可能性あり）
  let receiver = build_system(
    SYSTEM_A,
    NODE_A_PORT,
    Props::from_fn({
      let delta = full_delta.clone();
      move || GrainHub::new(delta.clone())
    })
    .with_name(HUB_NAME),
    None,
  )?;

  // ノードB（送信 + reply channel 登録）
  let sender = build_system(
    SYSTEM_B,
    NODE_B_PORT,
    Props::from_fn({
      let delta = full_delta.clone();
      move || GrainHub::new(delta.clone())
    })
    .with_name(HUB_NAME),
    Some(tx_shared.clone()),
  )?;

  println!("[info] receiver user guardian: {:?}", receiver.user_guardian_ref().path());
  println!("[info] sender   user guardian: {:?}", sender.user_guardian_ref().path());

  let ext_id =
    RemotingExtensionId::<StdToolbox>::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"));
  let _ = sender.extended().extension(&ext_id).expect("ext sender");
  let _ = receiver.extended().extension(&ext_id).expect("ext receiver");

  // owner を Rendezvous で決定
  let authorities = vec![format!("{HOST}:{NODE_A_PORT}"), format!("{HOST}:{NODE_B_PORT}")];
  let key = GrainKey::new(SAMPLE_KEY.to_string());
  let owner = RendezvousHasher::select(&authorities, &key).expect("owner");
  let (_owner_system, owner_ref) = if owner.ends_with(&NODE_A_PORT.to_string()) {
    (SYSTEM_A, receiver.user_guardian_ref())
  } else {
    (SYSTEM_B, sender.user_guardian_ref())
  };

  // Grain 呼び出し（返信先 ActorRef をメッセージに含める）
  let reply_to = sender.user_guardian_ref();
  owner_ref
    .tell(AnyMessage::new(GrainCall {
      key: SAMPLE_KEY.to_string(),
      body: "ping from quickstart".to_string(),
      reply_to,
    }))
    .map_err(|e| anyhow!("tell failed: {e:?}"))?;

  // 返信受信
  let reply = tokio::time::timeout(Duration::from_secs(5), rx)
    .await
    .map_err(|_| anyhow!("timeout waiting reply"))?
    .map_err(|_| anyhow!("reply channel dropped"))?;

  println!("[info] seeds       : {}", seeds.join(", "));
  println!("[info] owner       : {owner}");
  println!("[info] grain path  : {:?}", owner_ref.canonical_path());
  println!("[info] reply       : {reply}");
  println!("[info] 再実行時はプロセスを停止し、ポート {NODE_A_PORT}/{NODE_B_PORT} の競合を避けてください。");

  sender.terminate().ok();
  receiver.terminate().ok();
  thread::sleep(Duration::from_millis(200));
  Ok(())
}

fn validate_required(seeds: &[String]) -> Result<()> {
  if seeds.is_empty() { bail!("必須設定が不足しています: seeds") } else { Ok(()) }
}

fn build_system(
  system_name: &str,
  port: u16,
  hub_props: Props,
  responder_tx: Option<Arc<Mutex<Option<oneshot::Sender<String>>>>>,
) -> Result<ActorSystem> {
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
        .with_extension_installer(RemotingExtensionInstaller::new(
          RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"),
        )),
    );

  let system = ActorSystem::new_with_config(&hub_props, &system_config)
    .map_err(|e| anyhow!("actor system build failed: {e:?}"))?;

  if let Some(tx) = responder_tx {
    system
      .user_guardian_ref()
      .tell(AnyMessage::new(RegisterReplyChannel { tx }))
      .map_err(|e| anyhow!("failed to register reply channel: {e:?}"))?;
  }

  let id =
    RemotingExtensionId::<StdToolbox>::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"));
  let _extension = system.extended().extension(&id).expect("extension registered");

  Ok(system)
}

fn sanitize_key(key: &str) -> String {
  key.replace(['/', ':'], "_")
}

// === Actors ===

struct GrainHub {
  registry:        VirtualActorRegistry,
  owner_authority: String,
  reply_tx:        Option<Arc<Mutex<Option<oneshot::Sender<String>>>>>,
}

impl GrainHub {
  fn new(delta: MembershipDelta) -> Self {
    let mut membership = MembershipTable::new(3);
    membership.apply_delta(delta.clone());
    let owner_authority = membership.snapshot().entries.first().map(|r| r.authority.clone()).unwrap_or_default();

    Self { registry: VirtualActorRegistry::new(32, 60), owner_authority, reply_tx: None }
  }
}

impl Actor for GrainHub {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    println!("[info] grain hub started for {HUB_NAME}");
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(register) = message.downcast_ref::<RegisterReplyChannel>() {
      self.reply_tx = Some(register.tx.clone());
      return Ok(());
    }

    if let Some(reply) = message.downcast_ref::<GrainReply>() {
      if let Some(tx) = &self.reply_tx {
        if let Some(sender) = tx.lock().unwrap().take() {
          let _ = sender.send(reply.body.clone());
        }
      }
      return Ok(());
    }

    let Some(call) = message.downcast_ref::<GrainCall>() else {
      println!("[warn] grain hub received unsupported message");
      return Ok(());
    };

    println!("[info] grain hub received key={}, body={}", call.key, call.body);

    let key = GrainKey::new(call.key.clone());

    // 所有者判定（自身が owner でない場合は Unreachable を返す想定だが、サンプルなので強制 owner）
    let activation = self
      .registry
      .ensure_activation(&key, &[self.owner_authority.clone()], 0, false, None)
      .map_err(|e| ActorError::recoverable(format!("activation failed: {e:?}")))?;

    let child_name = sanitize_key(key.value());
    let props = Props::from_fn({
      let reply_to = call.reply_to.clone();
      let body = call.body.clone();
      move || GrainActor::new(reply_to.clone(), body.clone())
    })
    .with_name(child_name);
    ctx
      .spawn_child(&props.as_core())
      .map_err(|e| ActorError::recoverable(format!("spawn grain actor failed: {e:?}")))?;

    println!("[info] activation pid: {activation}");
    Ok(())
  }
}

struct GrainActor {
  reply_to: ActorRef,
  body:     String,
}

impl GrainActor {
  fn new(reply_to: ActorRef, body: String) -> Self {
    Self { reply_to, body }
  }

  fn send_reply(&self) -> Result<(), ActorError> {
    self
      .reply_to
      .tell(AnyMessage::new(GrainReply { body: format!("echo:{}", self.body) }))
      .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))
  }
}

impl Actor for GrainActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    println!("[info] grain spawned for reply");
    self.send_reply()
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.send_reply()
  }
}

#[derive(Clone, Debug)]
struct GrainCall {
  key:      String,
  body:     String,
  reply_to: ActorRef,
}

#[derive(Clone, Debug)]
struct GrainReply {
  body: String,
}

#[derive(Clone)]
struct RegisterReplyChannel {
  tx: Arc<Mutex<Option<oneshot::Sender<String>>>>,
}
