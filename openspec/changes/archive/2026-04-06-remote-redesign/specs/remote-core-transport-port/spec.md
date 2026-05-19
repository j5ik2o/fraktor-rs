## ADDED Requirements

### Requirement: RemoteTransport trait の存在

`fraktor_remote_core_rs::transport::RemoteTransport` trait が定義され、Pekko `RemoteTransport` (Scala abstract class, 113行) のメソッドセットに対応する API を提供する SHALL。

#### Scenario: trait の公開

- **WHEN** `modules/remote-core/src/transport.rs` および `transport/` サブモジュールを検査する
- **THEN** `pub trait RemoteTransport` が定義されている

#### Scenario: 1つの port のみ存在

- **WHEN** `modules/remote-core/src/transport/` 配下を検査する
- **THEN** transport 実装の trait は `RemoteTransport` 1つのみで、複数の transport trait が並列に存在しない

### Requirement: ライフサイクルメソッド

`RemoteTransport` trait は `start` および `shutdown` メソッドを `&mut self` で同期 API として持つ SHALL。

#### Scenario: start メソッドのシグネチャ

- **WHEN** `RemoteTransport` の定義を読む
- **THEN** `fn start(&mut self) -> Result<(), TransportError>` が宣言されている

#### Scenario: shutdown メソッドのシグネチャ

- **WHEN** `RemoteTransport` の定義を読む
- **THEN** `fn shutdown(&mut self) -> Result<(), TransportError>` が宣言されている

#### Scenario: async でない

- **WHEN** `RemoteTransport` のすべてのメソッドを検査する
- **THEN** どのメソッドも `async fn` ではなく、戻り値型に `Future`・`Pin<Box<dyn Future>>` を含まない

### Requirement: メッセージ送信メソッド

`RemoteTransport` trait は `send` メソッドを持ち、`OutboundEnvelope` 単位で送信を要求する SHALL。Pekko `RemoteTransport.send(message, sender, recipient)` に対応する。

#### Scenario: send メソッドのシグネチャ

- **WHEN** `RemoteTransport::send` の定義を読む
- **THEN** `fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), TransportError>` が宣言されている

#### Scenario: byte 単位ではなく envelope 単位

- **WHEN** `send` メソッドの引数型を確認する
- **THEN** 引数は `&[u8]` や `Bytes` ではなく `OutboundEnvelope` (recipient, sender, message, priority を含む構造体) である

### Requirement: アドレス取得メソッド

`RemoteTransport` trait は `addresses`、`default_address`、`local_address_for_remote` メソッドを持つ SHALL。Pekko `RemoteTransport.addresses` / `defaultAddress` / `localAddressForRemote` に対応する。

#### Scenario: addresses メソッド

- **WHEN** `RemoteTransport::addresses` の定義を読む
- **THEN** `fn addresses(&self) -> &[Address]` または同等のシグネチャが宣言されている (`&self` の query、CQS 準拠)

#### Scenario: default_address メソッド

- **WHEN** `RemoteTransport::default_address` の定義を読む
- **THEN** `fn default_address(&self) -> Option<&Address>` または同等のシグネチャが宣言されている

#### Scenario: local_address_for_remote メソッド

- **WHEN** `RemoteTransport::local_address_for_remote` の定義を読む
- **THEN** `fn local_address_for_remote(&self, remote: &Address) -> Option<&Address>` または同等のシグネチャが宣言されている

### Requirement: quarantine メソッド

`RemoteTransport` trait は `quarantine` メソッドを持ち、リモートシステムを quarantine 状態に遷移させる SHALL。Pekko `RemoteTransport.quarantine(address, uid, reason)` に対応する。

#### Scenario: quarantine メソッドのシグネチャ

- **WHEN** `RemoteTransport::quarantine` の定義を読む
- **THEN** `fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), TransportError>` または同等のシグネチャが宣言されている

### Requirement: TransportError 型

`fraktor_remote_core_rs::transport::TransportError` enum が定義され、トランスポート操作の失敗カテゴリを網羅する SHALL。

#### Scenario: TransportError の存在

- **WHEN** `modules/remote-core/src/transport/transport_error.rs` を検査する
- **THEN** `pub enum TransportError` が定義され、`UnsupportedScheme`、`NotAvailable`、`AlreadyRunning`、`NotStarted`、`SendFailed`、`ConnectionClosed` 等のバリアントを含む

#### Scenario: core::error::Error の実装

- **WHEN** `TransportError` のderive またはimpl ブロックを検査する
- **THEN** `Debug`、`Display`、`core::error::Error` (no_std 互換) が実装されている

### Requirement: ロックを返さない API

`RemoteTransport` の任意のメソッドは、`Guard`・`MutexGuard`・`RwLockReadGuard` 等のロックガード型を戻り値として返さない SHALL。

#### Scenario: ロックガードを返さない

- **WHEN** `RemoteTransport` のすべてのメソッドの戻り値型を検査する
- **THEN** どの戻り値にも `Guard`・`MutexGuard`・`RwLockReadGuard`・`SpinSyncMutexGuard` 等のロックガード型が含まれない
