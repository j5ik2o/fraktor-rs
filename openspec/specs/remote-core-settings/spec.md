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

