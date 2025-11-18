//! Minimal loopback remoting bootstrap demonstrated in tests as well.

use anyhow::Result;
use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPathParts, GuardianKind},
  },
  error::ActorError,
  event_stream::BackpressureSignal,
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemBuilder, ActorSystemGeneric, AuthorityState},
};
use fraktor_remote_rs::core::{
  FlightMetricKind, RemoteActorRefProvider, RemotingControl, RemotingControlHandle, RemotingExtensionConfig,
  RemotingExtensionId,
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

struct ExampleActor;

impl Actor<NoStdToolbox> for ExampleActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system(
  config: RemotingExtensionConfig,
) -> Result<(ActorSystemGeneric<NoStdToolbox>, RemotingControlHandle<NoStdToolbox>)> {
  let props = PropsGeneric::from_fn(|| ExampleActor).with_name("loopback-guardian");
  let extensions = ExtensionsConfig::default().with_extension_config(config.clone());
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .with_actor_ref_provider(RemoteActorRefProvider::loopback())
    .build()
    .map_err(|error| anyhow::anyhow!("{error}"))?;
  let id = RemotingExtensionId::<NoStdToolbox>::new(config);
  let extension = system.extension(&id).expect("extension registered");
  Ok((system, extension.handle()))
}

fn main() -> Result<()> {
  let config = RemotingExtensionConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(25520)
    .with_auto_start(false)
    .with_flight_recorder_capacity(32);

  let (system, handle) = build_system(config)?;
  handle.start().map_err(|error| anyhow::anyhow!("{error}"))?;

  let provider = system.actor_ref_provider::<RemoteActorRefProvider<NoStdToolbox>>().expect("provider installed");
  let parts =
    ActorPathParts::with_authority("remote-demo", Some(("127.0.0.1", 25520))).with_guardian(GuardianKind::User);
  provider.watch_remote(parts).map_err(|error| anyhow::anyhow!("{error}"))?;

  // 監視スナップショットを標準出力へ表示
  for snapshot in provider.connections_snapshot() {
    match snapshot.state().clone() {
      | AuthorityState::Connected => println!("authority={} state=connected", snapshot.authority()),
      | AuthorityState::Unresolved => println!("authority={} state=unresolved", snapshot.authority()),
      | AuthorityState::Quarantine { deadline } => {
        println!("authority={} state=quarantine deadline={:?}", snapshot.authority(), deadline)
      },
    }
  }

  // Backpressure シグナルを人工的に送出して Flight Recorder を観察
  handle.emit_backpressure_signal("127.0.0.1:25520", BackpressureSignal::Apply);
  let metrics = handle.flight_recorder_snapshot();
  for metric in metrics.records() {
    if let FlightMetricKind::Backpressure(signal) = metric.kind().clone() {
      println!("backpressure authority={} signal={:?}", metric.authority(), signal);
    }
  }

  drop(provider);
  drop(handle);
  drop(system);
  Ok(())
}
