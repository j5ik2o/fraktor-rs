# remote-adaptor-std-runtime Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: AssociationShared と AssociationRegistry

adapter 側に `AssociationShared` 型 (core の `Association` を共有可能にする薄いラッパー) と `AssociationRegistry` 型 (per-remote の `Association` を管理する BTreeMap) が定義される SHALL。これは core の `Association` が `&mut self` ベースで共有不可能であることを受けて、adapter 側で内部可変性を導入する唯一の点である。

#### Scenario: AssociationShared の存在

- **WHEN** `modules/remote-adaptor-std/src/association_runtime/association_shared.rs` を読む
- **THEN** `pub struct AssociationShared = ArcShared<SpinSyncMutex<Association>>` または同等の AShared パターン型が定義されている

#### Scenario: AssociationRegistry の存在

- **WHEN** `modules/remote-adaptor-std/src/association_runtime/association_registry.rs` を読む
- **THEN** `pub struct AssociationRegistry` が定義され、`BTreeMap<UniqueAddress, AssociationShared>` または同等の型を内部に保持する

### Requirement: outbound loop (送信 tokio task)

adapter 側に送信 tokio task が定義され、core の `Association::next_outbound()` を呼び出して取り出した envelope を `TcpRemoteTransport::send` に渡す SHALL。

#### Scenario: outbound loop の動作

- **WHEN** `AssociationShared` に対応する送信 loop が動作している状態で、`Association::enqueue` が呼ばれる
- **THEN** outbound loop は `next_outbound()` で envelope を取り出し、TCP 接続経由で送信する

### Requirement: inbound dispatch (受信 tokio task)

adapter 側に受信 tokio task が定義され、TCP から受信した frame を core の `Association` 関連メソッド (`handshake_accepted` 等) に渡して effect 列を実行する SHALL。

#### Scenario: 受信 frame の dispatch

- **WHEN** 受信 loop が `HandshakePdu::Rsp` を受信する
- **THEN** 対応する `AssociationShared` の `Association::handshake_accepted(remote_node, now_ms: u64 /* monotonic millis */)` が呼ばれ、戻り値の effect 列を順に実行する (deferred flush 等)

### Requirement: handshake driver (タイムアウト監視)

adapter 側に handshake driver tokio task が定義され、`tokio::time::sleep` で経過を計測し、タイムアウト時に core の `Association::handshake_timed_out(now_ms: u64 /* monotonic millis */, resume_at_ms)` を呼ぶ SHALL。

#### Scenario: handshake タイムアウト発火

- **WHEN** handshake driver が `handshake_timeout` 経過時点に到達する
- **THEN** `Association::handshake_timed_out(now_ms, Some(resume_at_ms))` が呼ばれ、戻り値の effect 列を実行する

#### Scenario: monotonic 時刻の使用

- **WHEN** handshake driver が core に時刻を渡す
- **THEN** `Instant::now()` の差分 (開始時点からの経過 ms) を `u64 /* monotonic millis */` として渡す。`SystemTime::now()` (wall clock) は使わない

### Requirement: system message delivery (ack-based redelivery)

adapter 側に system message の ack-based redelivery 実装が含まれる SHALL。これは core の `AckPdu` と組み合わせて動作し、sequence number 管理とリトライロジックを提供する。

#### Scenario: sequence number の管理

- **WHEN** system priority の envelope を送信するタイミング
- **THEN** 連番の sequence number が付与され、`AckPdu` を受信するまで retry 対象として管理される

#### Scenario: ack 受信による retry 停止

- **WHEN** `AckPdu { cumulative_ack }` を受信する
- **THEN** `cumulative_ack` 以下の sequence number の envelope は retry 対象から除外される

### Requirement: RemoteSettings の ack フィールド追加

Phase A では延期された `RemoteSettings::ack_send_window` と `ack_receive_window` フィールドは、本 Phase (adapter side の system message delivery 実装) と同時に `remote-core-settings` capability に追加される SHALL。

#### Scenario: ack フィールドの追加

- **WHEN** Phase B 完了時点の `RemoteSettings` のフィールドを検査する
- **THEN** `ack_send_window: u64` と `ack_receive_window: u64` が追加されている

### Requirement: WatcherActor (WatcherState の actor 化層)

adapter 側に `WatcherActor` が定義され、core の `WatcherState` を保持して tokio timer と actor messaging で駆動する SHALL。これは core の pure state machine を runtime に接続する薄い wiring layer である。

#### Scenario: WatcherActor の存在

- **WHEN** `modules/remote-adaptor-std/src/watcher_actor/watcher_actor.rs` を読む
- **THEN** `pub struct WatcherActor` が定義され、内部に `WatcherState` (core 由来) を保持する

#### Scenario: tokio timer による heartbeat tick 発火

- **WHEN** `WatcherActor` が tokio runtime 上で動作している
- **THEN** `tokio::time::interval` または同等の timer で定期的に `WatcherState::handle(WatcherCommand::HeartbeatTick { now: u64 /* monotonic millis */ })` が呼ばれ、戻り値の effect 列が実行される

#### Scenario: SendHeartbeat effect の実行

- **WHEN** `WatcherState::handle` の戻り値に `WatcherEffect::SendHeartbeat { to }` が含まれる
- **THEN** `WatcherActor` は `TcpRemoteTransport::send` または control channel 経由で heartbeat frame (`ControlPdu::Heartbeat`) を送信する

#### Scenario: core 側の時刻入力が monotonic

- **WHEN** `WatcherActor` が core の `WatcherState::handle` を呼ぶタイミングを検査する
- **THEN** `now` 引数には `tokio::time::Instant::now()` の差分 (または `std::time::Instant::now()` の差分) を millis 換算した値が渡され、wall clock (`SystemTime::now()`) は使われていない

### Requirement: StdRemoting と RemotingExtensionInstaller

adapter 側に `StdRemoting` 型と `RemotingExtensionInstaller` が定義され、core の `Remoting` trait を実装して actor system extension として組み込み可能である SHALL。`StdRemoting` は god object だった `RemotingControlHandle` の分散後の「runtime 配線層」であり、core の `Remoting` trait (pure lifecycle) + `TcpRemoteTransport` + `AssociationRegistry` + `WatcherActor` + `StdRemoteActorRefProvider` を1つに束ねる役割を担う。

#### Scenario: StdRemoting の存在

- **WHEN** `modules/remote-adaptor-std/src/extension_installer.rs` または `src/extension_installer/` を読む
- **THEN** `pub struct StdRemoting` が定義されている

#### Scenario: core Remoting trait の実装

- **WHEN** `StdRemoting` の trait 実装を検査する
- **THEN** `impl Remoting for StdRemoting` が存在し、core の `start`・`shutdown`・`quarantine`・`addresses` メソッドを実装している

#### Scenario: runtime 配線コンポーネントの保持

- **WHEN** `StdRemoting` のフィールドを検査する
- **THEN** `TcpRemoteTransport`・`AssociationRegistry`・`WatcherActor`・`StdRemoteActorRefProvider` を (直接または `Arc`/`Box` 経由で) 保持している

#### Scenario: core Remoting trait が runtime 配線を持たない

- **WHEN** core の `Remoting` trait メソッド一覧と `StdRemoting` の内部フィールドを比較する
- **THEN** runtime 配線 (`transport_ref`/`bridge_factory`/`watcher_daemon`/`heartbeat_channels` 等) は `StdRemoting` のみが持ち、core の `Remoting` trait 側には対応するメソッドが存在しない (Phase A の `remote-core-extension` capability 要件と整合)

#### Scenario: RemotingExtensionInstaller による actor system への登録

- **WHEN** `RemotingExtensionInstaller` の定義を読む
- **THEN** actor system extension として `StdRemoting` を登録し、lifecycle を actor system の起動/終了に同期する機構を提供している

#### Scenario: god object 分解結果の検証

- **WHEN** `StdRemoting` の責務と既存 `modules/remote/src/core/remoting_extension/control_handle.rs` (479行の god object) の責務を比較する
- **THEN** 旧 `RemotingControlHandle` の以下の責務が `StdRemoting` に集約されている: transport_ref 保持、writer/reader/bridge_factory/endpoint_bridge、watcher_daemon、heartbeat_channels (runtime 配線部)。一方、lifecycle state、flight recorder、backpressure listener、snapshots (純粋データ部) は core 側の `remote-core-extension` に配置されている

