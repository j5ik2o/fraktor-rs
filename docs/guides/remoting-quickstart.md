# Remoting Quickstart

RemotingExtension と RemoteActorRefProvider を組み合わせて、2 つの ActorSystem 間で LoopbackTransport を使った疎通確認を行うための手順をまとめます。ここでは no_std/Loopback 構成を前提にしていますが、Tokio/std 版でも同じ流れで `RemoteActorRefProvider::std()` を使うだけです。

## 1. RemotingExtension の設定

1. `ActorSystemConfig` の `RemotingConfig` で canonical host/port を定義します（システム全体の設定）。ここで指定したアドレスは **serialize/resolve 時に自動注入** され、TransportInformation が無い場合でも canonical URI が得られます。
2. `RemotingExtensionConfig` で Transport scheme や AutoStart などの拡張固有の設定を定義します。canonical host/port を省略すると、ActorSystemConfig の RemotingConfig から自動的に取得されます。bind アドレスを分けたい場合は transport 実装（例: TokioTransportConfig）側で設定し、公開用は RemotingConfig の canonical_host/port を優先します。
3. `ExtensionInstallers` に `with_extension_installer(RemotingExtensionInstaller::new(config))` で登録します。
4. `ActorSystemConfig` 側で TickDriver を必ずセットします。テストでは `TickDriverConfig::manual(ManualTestDriver::new())` がシンプルです。
5. feature flag 例（Tokio std 版）: `--features std,tokio-transport,tokio-executor`。Loopback/no_std 版は `std` を外せば動きます。

```rust
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  config::{ActorSystemConfig, RemotingConfig},
  extension::ExtensionInstallers,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{tick_driver::ManualTestDriver, TickDriverConfig},
  system::{ActorSystemGeneric, AuthorityState},
};
use fraktor_remote_rs::{
  LoopbackActorRefProvider, LoopbackActorRefProviderInstaller,
  RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

fn bootstrap(name: &str, port: u16) -> anyhow::Result<(
  ActorSystemGeneric<NoStdToolbox>,
  fraktor_remote_rs::RemotingControlHandle<NoStdToolbox>,
)> {
  let props = PropsGeneric::from_fn(|| Guardian).with_name(name);

  // 拡張設定: canonical host/port は省略（ActorSystemConfig から取得される）
  let remoting_config = RemotingExtensionConfig::default().with_auto_start(false);

  let extensions = ExtensionInstallers::default()
    .with_extension_installer(RemotingExtensionInstaller::new(remoting_config.clone()));

  // システム設定: ここで canonical host/port を設定
  let system_config = ActorSystemConfig::default()
    .with_system_name(name.to_string())
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_remoting_config(
      RemotingConfig::default()
        .with_canonical_host("127.0.0.1")
        .with_canonical_port(port)
    )
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_extension_installers(extensions);

  let system = ActorSystemGeneric::new_with_config(&props, &system_config)?;

  let id = RemotingExtensionId::new(remoting_config);
  let extension = system.extended().extension(&id).expect("extension registered");
  Ok((system, extension.handle()))
}
```

## 2. ActorRefProvider の取得と起動

`ActorSystemConfig` に `LoopbackActorRefProviderInstaller` を渡すと、起動後に `RemoteWatcherDaemon` が SystemGuardian 配下へ自動生成され、`ActorSystem::extended().actor_ref_provider::<LoopbackActorRefProvider<_>>()` からハンドルを取得できます。

1. `let provider = system.extended().actor_ref_provider::<LoopbackActorRefProvider<_>>().expect("provider installed");`
2. RemotingExtension のハンドルを取得して `start()` を呼び出します。
3. `provider.watch_remote(ActorPathParts::with_authority(...))` で watch を投げると `RemotingControl::associate` が動作し、`RemoteAuthorityManager` に Connected が反映されます。

```rust
let (system_a, handle_a) = bootstrap("system-a", 4100)?;
let provider_a = system_a.extended().actor_ref_provider::<LoopbackActorRefProvider<NoStdToolbox>>()
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

1. TickDriver を `TickDriverConfig::manual(ManualTestDriver::new())` から適切な Tokio 対応のものに置き換える。
2. Provider 設定を `LoopbackActorRefProviderInstaller` から `TokioActorRefProviderInstaller::from_config(TokioTransportConfig::default())` に変更する。
3. `RemotingExtensionConfig` で `with_transport_scheme("fraktor.tcp")` など Tokio 用の Transport を選択する（canonical host/port は ActorSystemConfig から自動取得される）。

```rust
let remoting_config = RemotingExtensionConfig::default()
  .with_transport_scheme("fraktor.tcp"); // 拡張固有の設定のみ
```

残りの watch/unwatch 呼び出しや FlightRecorder の API は loopback 版と同じです。

---

これで RemotingExtension/RemoteActorRefProvider を Builder に統合した最小構成が完成します。実装中の API は `modules/remote/tests/quickstart.rs` とこのガイドを同期更新してください。実際に手元で挙動を確かめたい場合は `modules/remote/examples/loopback_quickstart.rs` を参考にし、`cargo run -p fraktor-remote-rs --example loopback_quickstart --features tokio-executor` を実行することで 2 系統の LoopbackTransport を観察できます。
