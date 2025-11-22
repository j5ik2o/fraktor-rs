#![allow(clippy::print_stdout)]

//! Cluster quickstart (cluster-capable sample)
//! - Two nodes (4050/4051) using Membership/Gossip/VirtualActorRegistry
//! - Select an owner via Rendezvous and spawn keyed actors via VirtualActorRegistry
//! - Reply routing is confirmed via the Responder actor

use std::{
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

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
use fraktor_cluster_rs::core::{GrainKey, MembershipDelta, MembershipTable, RendezvousHasher, VirtualActorRegistry};
use fraktor_remote_rs::core::{
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller, TokioActorRefProviderGeneric,
  TokioActorRefProviderInstaller, TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use tokio::sync::oneshot;

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 4050;
const NODE_B_PORT: u16 = 4051;
const SYSTEM_A: &str = "cluster-receiver";
const SYSTEM_B: &str = "cluster-sender";
const HUB_NAME: &str = "grains";
const RESPONDER_NAME: &str = "responder";
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

  // ノードB（送信 + Responder）
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

  let provider_sender =
    sender.extended().actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>().expect("provider sender");

  // 双方向 watch で remoting を有効化
  provider_sender.watch_remote(receiver_authority_parts()).map_err(|e| anyhow!("sender watch failed: {e}"))?;
  tokio::time::sleep(Duration::from_millis(300)).await;
  let ext_id =
    RemotingExtensionId::<StdToolbox>::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"));
  let _ = sender.extended().extension(&ext_id).expect("ext sender");
  let _ = receiver.extended().extension(&ext_id).expect("ext receiver");

  // owner を Rendezvous で決定
  let authorities = vec![format!("{HOST}:{NODE_A_PORT}"), format!("{HOST}:{NODE_B_PORT}")];
  let key = GrainKey::new(SAMPLE_KEY.to_string());
  let owner = RendezvousHasher::select(&authorities, &key).expect("owner");
  let (owner_system, owner_port) =
    if owner.ends_with(&NODE_A_PORT.to_string()) { (SYSTEM_A, NODE_A_PORT) } else { (SYSTEM_B, NODE_B_PORT) };

  let grain_path = grain_actor_path(owner_system, owner_port, SAMPLE_KEY);
  let target_ref = provider_sender.actor_ref(grain_path).expect("grain actor ref");

  // 返信先を ActorPath で渡す
  let reply_path = ActorPathFormatter::format(&responder_path());
  let payload = format!("{SAMPLE_KEY}|{reply_path}|ping from quickstart");
  target_ref.tell(AnyMessage::new(payload)).map_err(|e| anyhow!("tell failed: {e:?}"))?;

  // Responder で返信を受信
  let reply = tokio::time::timeout(Duration::from_secs(5), rx)
    .await
    .map_err(|_| anyhow!("timeout waiting reply"))?
    .map_err(|_| anyhow!("responder dropped"))?;

  println!("[info] seeds       : {}", seeds.join(", "));
  println!("[info] owner       : {owner}");
  println!(
    "[info] grain path  : {}",
    ActorPathFormatter::format(&grain_actor_path(owner_system, owner_port, SAMPLE_KEY))
  );
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
    let responder_props = Props::from_fn(move || Responder::new(tx.clone())).with_name(RESPONDER_NAME);
    system
      .user_guardian_ref()
      .tell(AnyMessage::new(responder_props))
      .map_err(|e| anyhow!("failed to spawn responder: {e:?}"))?;
  }

  let id =
    RemotingExtensionId::<StdToolbox>::new(RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp"));
  let _extension = system.extended().extension(&id).expect("extension registered");

  Ok(system)
}

fn receiver_authority_parts() -> ActorPathParts {
  ActorPathParts::with_authority(SYSTEM_A, Some((HOST, NODE_A_PORT)))
}

fn grain_actor_path(system: &str, port: u16, _key: &str) -> ActorPath {
  ActorPath::from_parts(ActorPathParts::with_authority(system, Some((HOST, port))).with_guardian(GuardianKind::User))
    .child(HUB_NAME)
}

fn responder_path() -> ActorPath {
  ActorPath::from_parts(
    ActorPathParts::with_authority(SYSTEM_B, Some((HOST, NODE_B_PORT))).with_guardian(GuardianKind::User),
  )
  .child(HUB_NAME)
  .child(RESPONDER_NAME)
}

fn sanitize_key(key: &str) -> String {
  key.replace(['/', ':'], "_")
}

// === Actors ===

struct GrainHub {
  registry:        VirtualActorRegistry,
  owner_authority: String,
}

impl GrainHub {
  fn new(delta: MembershipDelta) -> Self {
    let mut membership = MembershipTable::new(3);
    membership.apply_delta(delta.clone());
    let owner_authority = membership.snapshot().entries.first().map(|r| r.authority.clone()).unwrap_or_default();

    Self { registry: VirtualActorRegistry::new(32, 60), owner_authority }
  }
}

impl Actor for GrainHub {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(props) = message.downcast_ref::<Props>() {
      ctx.spawn_child(&props.as_core()).map_err(|e| ActorError::recoverable(format!("spawn child failed: {e:?}")))?;
      return Ok(());
    }

    let Some(text) = message.downcast_ref::<String>() else {
      println!("[warn] grain hub received non-string");
      return Ok(());
    };

    let mut parts = text.splitn(3, '|');
    let key_str = parts.next().unwrap_or("");
    let reply_path_str = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");

    println!("[info] grain hub received key={key_str}, body={body}");

    let key = GrainKey::new(key_str.to_string());

    // 所有者判定（自身が owner でない場合は Unreachable を返す想定だが、サンプルなので強制 owner）
    let activation = self
      .registry
      .ensure_activation(&key, &[self.owner_authority.clone()], 0, false, None)
      .map_err(|e| ActorError::recoverable(format!("activation failed: {e:?}")))?;

    // Grain アクターを spawn / 再利用
    let child_name = sanitize_key(key.value());
    let props = Props::from_fn({
      let reply_path_str = reply_path_str.to_string();
      let body = body.to_string();
      move || GrainActor::new(reply_path_str.clone(), body.clone())
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
  reply_path: String,
  body:       String,
}

impl GrainActor {
  fn new(reply_path: String, body: String) -> Self {
    Self { reply_path, body }
  }

  fn send_reply(&self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    let Ok(reply_path) = ActorPathParser::parse(&self.reply_path) else {
      println!("[warn] invalid reply path: {}", self.reply_path);
      return Ok(());
    };

    if let Some(provider) = ctx.system().extended().actor_ref_provider::<TokioActorRefProviderGeneric<StdToolbox>>() {
      match provider.actor_ref(reply_path) {
        | Ok(reply_ref) => {
          reply_ref
            .tell(AnyMessage::new(format!("echo:{}", self.body)))
            .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
        },
        | Err(err) => println!("[warn] reply actor not found: {err:?}"),
      }
    }
    Ok(())
  }
}

impl Actor for GrainActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    println!("[info] grain spawned for reply -> {}", self.reply_path);
    self.send_reply(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.send_reply(ctx)
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
