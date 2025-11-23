#![allow(clippy::print_stdout)]

#[cfg(not(all(feature = "std", feature = "test-support")))]
compile_error!("loopback_quickstart example requires `--features std,test-support`");

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContext, actor_ref::ActorRef},
  error::ActorError,
  extension::ExtensionInstallers,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::SerializationExtensionInstaller,
  system::{ActorSystem, ActorSystemConfig, ActorSystemGeneric, RemotingConfig},
};
use fraktor_remote_rs::core::{
  LoopbackActorRefProviderInstaller, RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
  default_loopback_setup,
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

const HOST: &str = "127.0.0.1";
const RECEIVER_PORT: u16 = 25520;
const SENDER_PORT: u16 = 25521;
const RECEIVER_GUARDIAN_NAME: &str = "receiver-guardian";
const SENDER_GUARDIAN_NAME: &str = "sender-guardian";

fn main() -> Result<()> {
  let (receiver, receiver_driver) = build_loopback_system(
    "loopback-receiver",
    RECEIVER_PORT,
    Props::from_fn(ReceiverGuardian::new).with_name(RECEIVER_GUARDIAN_NAME),
    receiver_transport_config(),
  )?;
  println!(
    "receiver user guardian path: {}",
    receiver.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  let (sender, sender_driver) = build_loopback_system(
    "loopback-sender",
    SENDER_PORT,
    Props::from_fn(SenderGuardian::new).with_name(SENDER_GUARDIAN_NAME),
    sender_transport_config(),
  )?;
  println!(
    "sender user guardian path: {}",
    sender.user_guardian_ref().path().map(|path| path.to_string()).unwrap_or_else(|| "<unknown>".into())
  );

  pump_manual_drivers(&[&sender_driver, &receiver_driver], 10);

  sender
    .user_guardian_ref()
    .tell(AnyMessage::new(StartPing {
      // ローカルで取得した ActorRef をそのまま渡すだけ。canonical 付与とリモート配送はランタイムが自動処理する。
      target: receiver.user_guardian_ref(),
      text:   "ping over loopback remoting".to_string(),
    }))
    .map_err(|error| anyhow!("{error:?}"))?;

  pump_manual_drivers(&[&sender_driver, &receiver_driver], 40);
  thread::sleep(Duration::from_millis(200));

  drop(sender);
  drop(receiver);
  Ok(())
}

fn build_loopback_system(
  system_name: &str,
  canonical_port: u16,
  guardian: Props,
  transport_config: RemotingExtensionConfig,
) -> Result<(ActorSystem, ManualTestDriver<NoStdToolbox>)> {
  let manual_driver = ManualTestDriver::new();
  let driver_handle = manual_driver.clone();
  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::manual(manual_driver))
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(canonical_port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(transport_config.clone())),
    );
  let system = ActorSystemGeneric::new_with_config(&guardian, &system_config).map_err(|error| anyhow!("{error:?}"))?;
  let id = RemotingExtensionId::<NoStdToolbox>::new(transport_config);
  let _ = system.extended().extension(&id).expect("extension registered");
  Ok((system, driver_handle))
}

fn pump_manual_drivers(drivers: &[&ManualTestDriver<NoStdToolbox>], ticks: u32) {
  for _ in 0..ticks {
    for driver in drivers {
      let controller = driver.controller();
      controller.inject_and_drive(1);
    }
  }
}

fn receiver_transport_config() -> RemotingExtensionConfig {
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  RemotingExtensionConfig::default().with_transport_scheme("fraktor.loopback")
}

fn sender_transport_config() -> RemotingExtensionConfig {
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  RemotingExtensionConfig::default().with_transport_scheme("fraktor.loopback")
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
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
  target: ActorRef,
  text:   String,
}

#[derive(Clone, Debug)]
struct Ping {
  text:     String,
  reply_to: ActorRef,
}
