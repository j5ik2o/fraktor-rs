# remote-adaptor-std-public-surface Specification

## Purpose

Define the public API boundary for `fraktor-remote-adaptor-std-rs` so application code uses only the std remote adapter entrypoints and does not depend on runtime internals.

## Requirements

### Requirement: remote-adaptor-std の public API は利用者向け adapter 境界に限定される

`fraktor-remote-adaptor-std-rs` は、通常利用者が remote を有効化するために必要な型だけを public API として公開しなければならない（MUST）。runtime driver、TCP frame、watcher actor、association task、remote event receiver、remote actor ref sender、低レベル provider plumbing の内部部品は crate 外から import できてはならない（MUST NOT）。

利用者向け adapter 境界として少なくとも以下は public に残す（MUST）。

- `TcpRemoteTransport`
- `RemotingExtensionInstaller`
- `StdRemoteActorRefProviderInstaller` または同等の高レベル provider installer / config API

#### Scenario: runtime internal 型は crate 外から import できない

- **WHEN** external crate 相当の public surface test から `fraktor_remote_adaptor_std_rs` 配下を import する
- **THEN** 以下の型または関数は import できない
  - `TcpClient`
  - `TcpServer`
  - `WireFrame`
  - `WireFrameCodec`
  - `FrameCodecError`
  - `InboundFrameEvent`
  - `TokioMpscRemoteEventReceiver`
  - `run_inbound_dispatch`
  - `WatcherActor`
  - `WatcherActorHandle`
  - `SubmitError`
  - `run_heartbeat_loop`
  - `RemoteActorRefSender`
  - `PathRemoteActorRefProvider` または同等の低レベル remote-only provider 実装

#### Scenario: 利用者向け adapter 境界は crate 外から利用できる

- **WHEN** external crate 相当の public surface test から `TcpRemoteTransport`、`RemotingExtensionInstaller`、高レベル provider installer / config API を import する
- **THEN** それらの型は public API として利用できる
- **AND** `TcpRemoteTransport` と `RemoteConfig` を `RemotingExtensionInstaller::new(...)` に渡せる
- **AND** caller は `RemoteShared` を取り出すために `installer.remote()` を呼ぶ必要がない

### Requirement: TcpRemoteTransport は内部 TCP 実装型を public method signature に漏らしてはならない

`TcpRemoteTransport` の public inherent method は、`TcpClient`、`TcpServer`、`WireFrame`、`WireFrameCodec`、`InboundFrameEvent` などの内部型を戻り値または引数に含めてはならない（MUST NOT）。runtime driver 専用の操作は `pub(crate)` 以下にしなければならない（MUST）。

#### Scenario: TcpRemoteTransport の public method signature に内部型が現れない

- **WHEN** `modules/remote-adaptor-std/src/transport/tcp/base.rs` の `impl TcpRemoteTransport` を検査する
- **THEN** `pub fn` の signature に `TcpClient`、`TcpServer`、`WireFrame`、`WireFrameCodec`、`InboundFrameEvent` が現れない
- **AND** それらを扱う method は `pub(crate)`、private、または module 内部 helper である

#### Scenario: TcpRemoteTransport は RemoteTransport port 実装として利用できる

- **WHEN** `TcpRemoteTransport` と `RemoteConfig` を `RemotingExtensionInstaller::new(...)` に渡す
- **THEN** std adapter は `TcpRemoteTransport` を core `RemoteTransport` port 実装として `RemoteShared` に接続できる
- **AND** 利用者は TCP client/server/frame 型を直接扱う必要がない

### Requirement: StdRemoteActorRefProvider の低レベル配線は installer または config 側に隠蔽される

`StdRemoteActorRefProvider` は actor-core の `ActorRefProvider` と remote-core の `RemoteActorRefProvider` を接続する adapter bridge として扱う（MUST）。ただし、通常利用者に `local_provider`、`remote_provider`、`event_sender`、`resolve_cache`、`event_publisher`、monotonic epoch を直接渡させる public constructor を公開してはならない（MUST NOT）。

remote actor ref provider の組み立ては extension installer、actor system configuration、またはそれに準じる高レベル builder が担当しなければならない（MUST）。

#### Scenario: StdRemoteActorRefProvider の低レベル constructor は public API に存在しない

- **WHEN** external crate 相当の public surface test から `StdRemoteActorRefProvider::new(...)` を呼び出そうとする
- **THEN** その低レベル constructor は利用できない
- **AND** `LocalActorRefProvider`、`RemoteActorRefProvider`、`TcpRemoteTransport`、`ActorRefResolveCache`、`EventPublisher` を手動で組み合わせる必要がない

#### Scenario: remote provider bridge は actor system 配線経由で構築される

- **WHEN** 利用者が remote extension installer または remote std configuration を actor system に指定する
- **THEN** std adapter は必要な local provider、remote provider、transport、resolve cache、event publisher を内部で組み立てる
- **AND** remote actor ref resolution は既存と同じ local loopback / remote dispatch 規則に従う

### Requirement: remote lifecycle control は user-facing main に露出しない

通常利用者は、`RemotingExtensionInstaller::new(transport, remote_config)` を shared extension installer として `ActorSystemConfig::with_extension_installers` に渡すだけで remote を有効化できなければならない（MUST）。user-facing code は install 後に `installer.remote()` から `RemoteShared` を取得して `remote.start()` を呼んではならない（MUST NOT）。user-facing code は remote run task 起動や shutdown join のために `spawn_run_task()` / `shutdown_and_join()` を直接呼んではならない（MUST NOT）。

#### Scenario: config install だけで remote lifecycle が開始される

- **GIVEN** caller が `TcpRemoteTransport` と `RemoteConfig` から `RemotingExtensionInstaller` を作成している
- **AND** caller がその installer を `ActorSystemConfig::with_extension_installers` に渡している
- **WHEN** `ActorSystem::create_with_config` が成功する
- **THEN** std adapter は core の `RemoteShared::start()` または同等の lifecycle operation を内部で呼ぶ
- **AND** std adapter は remote event receiver を使う run task を内部で起動する
- **AND** caller は `installer.remote()?.start()` または `installer.spawn_run_task()` を呼ばない

#### Scenario: ActorSystem termination が remote shutdown と join を起動する

- **GIVEN** remote extension installer が ActorSystem に install 済みである
- **WHEN** caller が ActorSystem termination を要求する
- **THEN** std adapter は core の shutdown semantics を内部で呼ぶ
- **AND** std adapter は event loop を wake し、tokio run task の完了を観測する
- **AND** caller は `installer.shutdown_and_join().await` を通常利用 path で呼ばない

#### Scenario: remote handle accessor は startup API として扱わない

- **WHEN** `RemotingExtensionInstaller::remote()` が診断または内部テスト用に残る
- **THEN** public showcase / docs / public surface test はそれを remote startup sequence として使わない
- **AND** `remote.addresses()` の確認は application main ではなく adapter / core tests に置かれる

### Requirement: showcase は runtime internal を直接 import してはならない

`showcases/std` の remote showcase は、runtime internal 型を直接 import してはならない（MUST NOT）。remote lifecycle と remote routee expansion の例は、利用者向け public API だけを示さなければならない（MUST）。

#### Scenario: remote lifecycle showcase は config install 境界だけを使う

- **WHEN** `showcases/std/legacy/remote_lifecycle/main.rs` または後継 remote lifecycle showcase を検査する
- **THEN** `TcpRemoteTransport`、`RemotingExtensionInstaller`、設定型以外の remote-adaptor runtime internal を import しない
- **AND** `TcpClient`、`TcpServer`、`WireFrame`、`WatcherActor` を参照しない
- **AND** `installer.remote()`、`remote.start()`、`spawn_run_task()`、`shutdown_and_join()` を remote startup / shutdown 手順として呼ばない

#### Scenario: remote routee expansion showcase は provider 低レベル constructor を呼ばない

- **WHEN** remote routee expansion showcase または後継 public surface test を検査する
- **THEN** `StdRemoteActorRefProvider::new(...)` を直接呼ばない
- **AND** routee expansion は installer / actor system / high-level provider API 経由で示される

### Requirement: 内部 runtime tests は crate 内部に配置される

association runtime、watcher actor、TCP frame codec などの詳細挙動は crate 内部テストで検証しなければならない（MUST）。crate 外 integration test は public API 契約だけを検証しなければならない（MUST）。

#### Scenario: internal 型の詳細テストは src 配下の module tests に残る

- **WHEN** association runtime、watcher actor、TCP frame codec の詳細挙動を検証する
- **THEN** test は `modules/remote-adaptor-std/src/**/tests.rs` または同等の crate 内部テストに置かれる
- **AND** external integration test は private module に依存しない

#### Scenario: public surface test は公開 API のみを使う

- **WHEN** public surface test を実行する
- **THEN** `TcpRemoteTransport`、`RemotingExtensionInstaller`、高レベル provider configuration などの public API だけを使う
- **AND** runtime internal 型を import しない
