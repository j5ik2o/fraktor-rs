# remote-core-transport-port Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: RemoteTransport trait の存在

`fraktor_remote_core_rs::domain::transport::RemoteTransport` trait が定義され、Pekko `RemoteTransport` (Scala abstract class, 113行) のメソッドセットに対応する API を提供する SHALL。

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

`RemoteTransport` trait は `send` メソッドを持ち、`OutboundEnvelope` 単位で送信を要求する SHALL。Pekko `RemoteTransport.send(message, sender, recipient)` に対応する。失敗時は caller が再 enqueue できるよう、元の `OutboundEnvelope` を error と一緒に返さなければならない（MUST）。

#### Scenario: send メソッドのシグネチャ

- **WHEN** `RemoteTransport::send` の定義を読む
- **THEN** `fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)>` または同等の元 envelope を返すシグネチャが宣言されている
- **AND** live OpenSpec は `Result<(), TransportError>` だけを要求してはならない

#### Scenario: byte 単位ではなく envelope 単位

- **WHEN** `send` メソッドの引数型を確認する
- **THEN** 引数は `&[u8]` や `Bytes` ではなく `OutboundEnvelope` (recipient, sender, message, priority を含む構造体) である
- **AND** wire bytes への変換は transport adapter 実装の責務であり、core port の引数型を raw bytes に変更しない

#### Scenario: error path は ownership を保持する

- **WHEN** transport が running でない、peer connection がない、または payload serialization に失敗する
- **THEN** `send` は error と元 envelope を返す
- **AND** caller は clone なしで元 envelope を retry queue に戻せる

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

`fraktor_remote_core_rs::domain::transport::TransportError` enum が定義され、トランスポート操作の失敗カテゴリを網羅する SHALL。

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

### Requirement: schedule_handshake_timeout メソッド

`RemoteTransport` trait は handshake request を送信する method と handshake timeout を adapter 側に予約させる method を持たなければならない（MUST）。`Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する際、handshake request frame の送信（`send_handshake`）に続けて timeout 予約 method を呼び、adapter 側 timer task を起動する。

#### Scenario: send_handshake メソッドのシグネチャ

- **WHEN** `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait 定義を読む
- **THEN** `fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError>` または同等のシグネチャが宣言されている
- **AND** メソッドは同期 `&mut self` で、`async fn` ではない（既存 trait 契約と整合）

#### Scenario: メソッドのシグネチャ

- **WHEN** `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait 定義を読む
- **THEN** `fn schedule_handshake_timeout(&mut self, authority: &TransportEndpoint, timeout: core::time::Duration, generation: u64) -> Result<(), TransportError>` または同等のシグネチャが宣言されている
- **AND** メソッドは同期 `&mut self` で、`async fn` ではない（既存 trait 契約と整合）

#### Scenario: 引数のセマンティクス

- **WHEN** `schedule_handshake_timeout` の rustdoc を読む
- **THEN** `authority` は handshake 対象の remote authority、`timeout` は満了までの時間、`generation` は `Association` が当該 handshake 試行に発行した単調増加 `u64` 値であることが明記されている
- **AND** adapter 実装は満了時に monotonic 時刻を取得し、`RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }` を adapter 内部 sender 経由で receiver に push する責務を持つ

#### Scenario: 同 authority への重複呼出

- **WHEN** 同じ `authority` に対して `schedule_handshake_timeout` を 2 回連続で呼ぶ
- **THEN** 2 回目の呼出は前回の timer task をキャンセルせず、独立した task として動作してよい
- **AND** 古い timer が満了して push する `HandshakeTimerFired` は `generation` 値が古いため `Remote::handle_remote_event` 側で `!=` 判定により破棄される（adapter 側でキャンセル責務を負わない）

#### Scenario: 戻り値の握りつぶし禁止

- **WHEN** `Remote::handle_remote_event` が `schedule_handshake_timeout` を呼ぶ実装を検査する
- **THEN** 戻り値の `Result<(), TransportError>` は `?` で伝播するか `match` で観測する
- **AND** `let _ = ...` で握りつぶしている経路が存在しない

### Requirement: handshake 関連 method 以外の timer 予約 API を本 change で追加しない

quarantine timer / large message ack timer 等、`schedule_handshake_timeout` 以外の遅延発火 API を `RemoteTransport` に追加してはならない（MUST NOT）。これらが必要になった時点で別 change として scheduling 経路を確定する。

#### Scenario: 他 timer 系 method の不在

- **WHEN** `RemoteTransport` trait のメソッド一覧を検査する
- **THEN** `schedule_quarantine_timer` / `schedule_drain_timer` / `schedule_event` など、handshake 以外の timer 予約 method が宣言されていない
- **AND** Timer Port 相当の汎用 trait（`pub trait Timer` 等）も存在しない

#### Scenario: スケジューリング経路の限定理由（spec 内 rationale）

- **WHEN** scheduling 系 API を一般化したい衝動が生じる
- **THEN** 本 change のスコープが「handshake timer の adapter 責務化」であり、QuarantineTimer / OutboundFrameAcked / BackpressureCleared 等の event は scheduling 経路と一緒に別 change で追加する方針であることを根拠に却下する
- **AND** YAGNI 原則と純増ゼロ方針を維持する

### Requirement: RemoteTransport の non-reentry / bounded-return 制約

`RemoteTransport` trait の同期 method（`send` / `send_handshake` / `schedule_handshake_timeout` / `shutdown` / `addresses` / `quarantine` 等）は、`RemoteShared` / `Remote` への **再入を行ってはならない**（MUST NOT）。各 method は **bounded 時間内に return しなければならない**（MUST、無限ブロックは禁止）。

これは共有実行時に `RemoteShared::run` の per-event lock（`with_write` クロージャ内）から `Remote::handle_remote_event` 経由で呼ばれるため、再入や無限ブロックがあると：

- 再入: 同一 `SharedLock` を二重に取得してデッドロック
- 無限ブロック: lock 区間が無限化し、他の `RemoteShared` clone からの `Remoting` 呼び出しが永遠に進行できない

#### Scenario: RemoteTransport 実装は再入しない

- **WHEN** `RemoteTransport` の任意の同期 method 実装を検査する
- **THEN** `RemoteShared::run` / `RemoteShared::start` / `RemoteShared::shutdown` / `RemoteShared::quarantine` / `RemoteShared::addresses` を直接呼ぶ経路が存在しない
- **AND** `Remote::handle_remote_event` / `Remote::start` / `Remote::shutdown` / `Remote::quarantine` 等の `Remote` 自身の method を直接呼ぶ経路も存在しない
- **AND** `SharedLock<Remote>` をキャプチャして `with_write` / `with_read` を呼ぶ経路も存在しない

#### Scenario: RemoteTransport method は bounded 時間内に return する

- **WHEN** `RemoteTransport` の任意の同期 method 実装を検査する
- **THEN** 無限ループ / 無条件 `loop {}` / 終了条件のない `while` を持たない
- **AND** 同期 I/O（TCP write 等）は OS タイムアウトに依存して bounded となるか、実装側でタイムアウトを設定する
- **AND** `tokio::spawn(async { ... })` のような fire-and-forget な経路は許容される（spawn 自体は即座に return するため bounded）

#### Scenario: 再入や無限ブロックの帰結

- **WHEN** `RemoteTransport` 実装が上記制約に違反する（例: `send` 内で `RemoteShared::shutdown` を呼ぶ）
- **THEN** デッドロックや並行性吸収の破綻が発生する
- **AND** これは `RemoteTransport` 実装側の責任であり、`RemoteShared` / `Remote` 側はその場合の動作を保証しない
