# remote-core-settings Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: RemoteSettings 型

`fraktor_remote_core_rs::settings::RemoteSettings` 型が定義され、リモートサブシステムの設定を型付き struct として保持する SHALL。Pekko の HOCON ベース `RemoteSettings` に対応するが、Rust では型付き struct + builder パターンで実装する。

#### Scenario: 型の存在

- **WHEN** `modules/remote-core/src/settings/remote_settings.rs` を読む
- **THEN** `pub struct RemoteSettings` が定義されている

#### Scenario: Phase A で必要なフィールドの保持

- **WHEN** Phase A 完了時点の `RemoteSettings` のフィールドを検査する
- **THEN** `canonical_host: String`・`canonical_port: Option<u16>`・`handshake_timeout: Duration`・`shutdown_flush_timeout: Duration`・`flight_recorder_capacity: usize` を含む

#### Scenario: Phase B で追加される ack 関連フィールド

- **WHEN** Phase B 完了時点の `RemoteSettings` のフィールドを検査する
- **THEN** `ack_send_window: u32`・`ack_receive_window: u32` が追加されており、それぞれ `with_ack_send_window` / `with_ack_receive_window` builder メソッドと accessor を持つ。adapter 側 `system_message_delivery` 機構と組み合わせて使う

### Requirement: コンストラクタ

`RemoteSettings::new` コンストラクタは必須項目 (canonical_host) のみを引数に取り、optional 項目はデフォルト値で初期化する SHALL。

#### Scenario: new シグネチャ

- **WHEN** `RemoteSettings::new` の定義を読む
- **THEN** `pub fn new(canonical_host: impl Into<String>) -> Self` または同等のシグネチャが宣言されている

#### Scenario: デフォルト値の適用

- **WHEN** `RemoteSettings::new("localhost")` を呼ぶ
- **THEN** `canonical_port` は `None`、`handshake_timeout` は合理的なデフォルト値 (例: 15秒)、`flight_recorder_capacity` は非ゼロのデフォルト値に設定される

### Requirement: Builder パターン API

optional フィールドは `with_*` プレフィックスの builder メソッドで変更可能である SHALL。これらは `self` を consume して新しい `RemoteSettings` を返し、method chain を可能にする。

#### Scenario: with_canonical_port

- **WHEN** `RemoteSettings::new("localhost").with_canonical_port(8080)` を呼ぶ
- **THEN** 戻り値の `canonical_port` が `Some(8080)` である

#### Scenario: method chain

- **WHEN** `RemoteSettings::new("localhost").with_canonical_port(8080).with_handshake_timeout(Duration::from_secs(30))` を呼ぶ
- **THEN** 両方の変更が適用された `RemoteSettings` が返る

#### Scenario: 元のインスタンスは変更されない (immutable builder)

- **WHEN** `let a = RemoteSettings::new("localhost"); let b = a.clone().with_canonical_port(8080);` を実行する
- **THEN** `a.canonical_port()` は `None` のまま、`b.canonical_port()` は `Some(8080)` である

### Requirement: accessor メソッド

すべての設定項目は `&self` の accessor メソッドで参照可能である SHALL。直接フィールドアクセスは `pub` にしない (encapsulation 維持)。

#### Scenario: canonical_host accessor

- **WHEN** `RemoteSettings::canonical_host()` の定義を読む
- **THEN** `fn canonical_host(&self) -> &str` が宣言されている

#### Scenario: フィールドが pub でない

- **WHEN** `RemoteSettings` struct のフィールド定義を検査する
- **THEN** すべてのフィールドは `pub` 修飾子を持たない

### Requirement: no_std 互換

`RemoteSettings` および関連型は `std` に依存せず、`core::time::Duration` と `alloc::string::String` のみで動作する SHALL。

#### Scenario: std 不依存

- **WHEN** `modules/remote-core/src/settings/` 配下の全 import を検査する
- **THEN** `use std::` を含む行が存在しない

### Requirement: Artery advanced settings surface

`fraktor_remote_core_rs::config::RemoteConfig` は Pekko Artery advanced settings の responsibility parity に必要な large-message、lane、inbound restart、compression の設定を型付き builder と accessor で保持する SHALL。これらの設定は fraktor-rs 独自 wire format の設定 surface であり、Pekko Artery wire protocol との byte compatibility を意味しない。

large-message と lane 設定は、設定保持だけでなく `Association` および std TCP transport の送受信処理へ反映されなければならない (MUST)。compression 設定は actor ref / serializer manifest compression table の sizing と advertisement scheduling に反映されなければならない (MUST)。payload bytes の圧縮はこの設定 surface の責務ではない (MUST NOT)。

#### Scenario: large-message 設定を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、large-message 用 builder を呼ぶ
- **THEN** outbound large-message queue size と large-message destinations が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid large-message queue size を拒否する

- **WHEN** outbound large-message queue size に `0` を指定する
- **THEN** `RemoteConfig` は invalid queue size として拒否し、zero-sized queue を保持しない

#### Scenario: large-message settings は Association enqueue policy に渡される

- **WHEN** `Association::from_config(local, remote, &config)` を呼ぶ
- **THEN** `config.large_message_destinations()` と `config.outbound_large_message_queue_size()` は association の enqueue policy に反映される

#### Scenario: lane settings を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、`with_inbound_lanes` / `with_outbound_lanes` を呼ぶ
- **THEN** inbound lane count と outbound lane count が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid lane count を拒否する

- **WHEN** inbound lane count または outbound lane count に `0` を指定する
- **THEN** `RemoteConfig` は invalid lane count として拒否し、zero lane を保持しない

#### Scenario: lane settings は TCP transport construction に渡される

- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** `config.inbound_lanes()` と `config.outbound_lanes()` は transport の inbound dispatch / outbound writer lane 構成に反映される

#### Scenario: inbound restart budget を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、inbound restart 用 builder を呼ぶ
- **THEN** inbound restart timeout と inbound max restarts が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression 設定 surface を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、compression 用 builder を呼ぶ
- **THEN** compression settings が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression max は table sizing に反映される

- **GIVEN** `RemoteConfig` に `actor_ref_max = Some(64)` と `manifest_max = Some(32)` を持つ compression settings を設定する
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** transport の peer-local actor-ref compression table configuration は最大 64 entries として保持される
- **AND** peer-local manifest compression table configuration は最大 32 entries として保持される

#### Scenario: compression max None は local outbound を kind 単位で無効化する

- **GIVEN** `RemoteConfig` に `actor_ref_max = None` と `manifest_max = Some(32)` を持つ compression settings を設定する
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** actor-ref local outbound compression は disabled になる
- **AND** manifest local outbound compression は enabled のまま保持される

#### Scenario: advertisement interval は std transport timer に渡される

- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** `config.compression_config().actor_ref_advertisement_interval()` と `manifest_advertisement_interval()` は transport の compression advertisement timer 構成に反映される

#### Scenario: payload bytes は compression table で置換しない

- **WHEN** compression settings を `RemoteConfig` に設定する
- **THEN** transport は serializer id / manifest / actor path metadata のみを compression table の対象にする
- **AND** serialized payload bytes を compression table entry または table reference に置換しない

#### Scenario: no_std 境界を維持する

- **WHEN** `modules/remote-core/src/config/` 配下の import を検査する
- **THEN** `use std::` を含む行は存在せず、advanced settings は `core` と `alloc` の範囲で表現されている

### Requirement: shutdown flush timeout is the flush deadline source

`RemoteConfig::shutdown_flush_timeout` は shutdown flush と DeathWatch notification 前 flush の deadline source として使われる SHALL。core state machine には timeout 値そのもの、または timeout から計算された monotonic deadline を caller が渡し、core は wall clock を参照してはならない（MUST NOT）。

#### Scenario: default timeout is used by flush drivers

- **WHEN** caller が `RemoteConfig::new(...)` を使い、flush timeout を明示設定しない
- **THEN** shutdown flush driver と DeathWatch 前 flush driver は default `shutdown_flush_timeout` を使う

#### Scenario: configured timeout is used by both flush paths

- **GIVEN** `RemoteConfig::new(...).with_shutdown_flush_timeout(Duration::from_secs(10))`
- **WHEN** shutdown flush または DeathWatch notification 前 flush を開始する
- **THEN** flush session deadline は 10 秒の timeout を基準に計算される

#### Scenario: zero timeout does not wait forever

- **GIVEN** `shutdown_flush_timeout` が `Duration::ZERO` である
- **WHEN** flush driver が flush session を開始する
- **THEN** driver は無限待機しない
- **AND** flush は即時 timeout として扱われ、shutdown または DeathWatch notification の後続処理へ進む

#### Scenario: core does not read wall clock

- **WHEN** `modules/remote-core/src/association/` と `modules/remote-core/src/watcher/` の flush timeout 処理を検査する
- **THEN** `Instant::now()`、`SystemTime::now()`、`std::time::` を直接参照しない
- **AND** monotonic millis は std adaptor から入力される
