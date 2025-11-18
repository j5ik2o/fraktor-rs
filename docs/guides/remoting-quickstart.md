# Remoting Quickstart

RemotingExtension と RemoteActorRefProvider を組み合わせて、2 つの ActorSystem 間で LoopbackTransport を使った疎通確認を行うための手順をまとめます。ここでは no_std/Loopback 構成を前提にしていますが、Tokio/std 版でも同じ流れで `RemoteActorRefProvider::std()` を使うだけです。

## 1. RemotingExtension の設定

1. `RemotingExtensionConfig` で canonical host/port、AutoStart を定義します。
2. `ExtensionsConfig` に `with_extension_config(remoting_config)` で登録します（Builder が自動登録します）。
3. `ActorSystemBuilder` 側で TickDriver を必ずセットします。テストでは `TickDriverConfig::manual(ManualTestDriver::new())` がシンプルです。

```rust
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{tick_driver::ManualTestDriver, TickDriverConfig},
  system::{ActorSystemBuilder, AuthorityState},
};
use fraktor_remote_rs::{RemoteActorRefProvider, RemotingExtensionConfig, RemotingExtensionId};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

fn bootstrap(name: &str, port: u16) -> anyhow::Result<(
  fraktor_actor_rs::core::system::ActorSystemGeneric<NoStdToolbox>,
  fraktor_remote_rs::RemotingControlHandle<NoStdToolbox>,
)> {
  let props = PropsGeneric::from_fn(|| Guardian).with_name(name);
  let remoting_config = RemotingExtensionConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(port)
    .with_auto_start(false);
  let extensions = ExtensionsConfig::default().with_extension_config(remoting_config.clone());
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .with_actor_ref_provider(RemoteActorRefProvider::loopback())
    .build()?;

  let id = RemotingExtensionId::new(remoting_config);
  let extension = system.register_extension(&id);
  Ok((system, extension.handle()))
}
```

## 2. RemoteActorRefProvider の取得と起動

Builder に `with_actor_ref_provider(RemoteActorRefProvider::loopback())` を渡すと、起動後に `RemoteWatcherDaemon` が SystemGuardian 配下へ自動生成され、`ActorSystem::actor_ref_provider::<RemoteActorRefProvider<_>>()` からハンドルを取得できます。

1. `let provider = system.actor_ref_provider::<RemoteActorRefProvider<_>>().unwrap();`
2. RemotingExtension のハンドルを取得して `start()` を呼び出します。
3. `provider.watch_remote(ActorPathParts::with_authority(...))` で watch を投げると `RemotingControl::associate` が動作し、`RemoteAuthorityManager` に Connected が反映されます。

```rust
let (system_a, handle_a) = bootstrap("system-a", 4100)?;
let provider_a = system_a.actor_ref_provider::<RemoteActorRefProvider<NoStdToolbox>>()
  .expect("provider installed");

handle_a.start()?;
let target = ActorPathParts::with_authority("system-b", Some(("127.0.0.1", 4200)));
provider_a.watch_remote(target)?;

let state = system_a.state().remote_authority_state("127.0.0.1:4200");
assert!(matches!(state, AuthorityState::Connected));
```

## 3. 観測とトラブルシュート

- `provider.connections_snapshot()` で FlightRecorder に蓄積された authority ごとの状態を即座に確認できます。
- `handle.flight_recorder_for_test().traces_snapshot()` で Backpressure や associate 時の CorrelationId を収集できます。
- EventStream には `RemotingLifecycleEvent::ListenStarted/Connected`、`RemotingBackpressureEvent` が流れるため、監視サブスクライバに hook すると CLI でも状態を追えます。
- Quickstart の E2E は `modules/remote/tests/quickstart.rs` を参照してください。2 系統の ActorSystem を立ち上げ、`watch_remote`→`connections_snapshot`→Backpressure シグナル→FlightRecorder snapshot と段階的に検証しています。

## 4. std/Tokio 版への切り替え

Tokio を使う場合は:

1. TickDriver を `StdTickDriverConfig::tokio_quickstart()` に置き換える。
2. Provider 設定を `with_actor_ref_provider(RemoteActorRefProvider::std())` に変更する。
3. `RemotingExtensionConfig` で `with_transport_scheme("fraktor.tcp")` など std 用の Transport を選択する。

残りの watch/unwatch 呼び出しや FlightRecorder の API は loopback 版と同じです。

---

これで RemotingExtension/RemoteActorRefProvider を Builder に統合した最小構成が完成します。実装中の API は `modules/remote/tests/quickstart.rs` とこのガイドを同期更新してください。
