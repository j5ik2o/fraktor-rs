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

`fraktor_remote_core_rs::config::RemoteConfig` は Pekko Artery advanced settings の responsibility parity に必要な large-message、inbound restart、compression の設定を型付き builder と accessor で保持する SHALL。これらの設定は fraktor-rs 独自 wire format の設定 surface であり、Pekko Artery wire protocol との byte compatibility を意味しない。

#### Scenario: large-message 設定を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、large-message 用 builder を呼ぶ
- **THEN** outbound large-message queue size と large-message destinations が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: invalid large-message queue size を拒否する

- **WHEN** outbound large-message queue size に `0` を指定する
- **THEN** `RemoteConfig` は invalid queue size として拒否し、zero-sized queue を保持しない

#### Scenario: inbound restart budget を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、inbound restart 用 builder を呼ぶ
- **THEN** inbound restart timeout と inbound max restarts が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: compression 設定 surface を保持する

- **WHEN** `RemoteConfig::new("localhost")` を作成し、compression 用 builder を呼ぶ
- **THEN** compression settings が `RemoteConfig` 内に保持され、対応する accessor で参照できる

#### Scenario: wire-level compression は有効化しない

- **WHEN** compression settings を `RemoteConfig` に設定する
- **THEN** `core/wire` の PDU encoding、TCP framing、compression table の wire 表現は変更されず、設定保持だけが行われる

#### Scenario: no_std 境界を維持する

- **WHEN** `modules/remote-core/src/config/` 配下の import を検査する
- **THEN** `use std::` を含む行は存在せず、advanced settings は `core` と `alloc` の範囲で表現されている
