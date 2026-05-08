# remote-adaptor-std-tcp-transport Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: TcpRemoteTransport 型

`fraktor_remote_adaptor_std_rs::std::transport::tcp::TcpRemoteTransport` 型が定義され、core の `RemoteTransport` trait を実装する SHALL。TCP ベースの std remote transport として、start / shutdown / handshake / control / envelope delivery / connection-loss notification を adapter runtime に接続する。

#### Scenario: 型の存在

- **WHEN** `modules/remote-adaptor-std/src/std/transport/tcp/base.rs` を読む
- **THEN** `pub struct TcpRemoteTransport` が定義されている

#### Scenario: RemoteTransport trait の実装

- **WHEN** `TcpRemoteTransport` の trait 実装を検査する
- **THEN** `impl RemoteTransport for TcpRemoteTransport` が存在し、core の全メソッド (`start`, `shutdown`, `send`, `send_control`, `send_handshake`, `schedule_handshake_timeout`, `addresses`, `default_address`, `local_address_for_remote`, `quarantine`) を実装している

### Requirement: bind と accept loop

`TcpRemoteTransport::start` は `tokio::net::TcpListener::bind` でリスナーを開始し、accept loop の tokio task を spawn する SHALL。

#### Scenario: bind 成功後に accept loop が動作

- **WHEN** `TcpRemoteTransport::new(config)` で作成し `start()` を呼ぶ
- **THEN** 指定されたアドレスで `TcpListener` が bind され、accept loop の tokio task が生成される

#### Scenario: bind 失敗時のエラー

- **WHEN** 既に使用中のポートに対して `start()` を呼ぶ
- **THEN** `Err(TransportError::SendFailed)` または同等のエラーが返る

### Requirement: outbound envelope 配送

`TcpRemoteTransport::send` は running 状態かつ対象 peer の writer が存在する場合、`OutboundEnvelope` を `EnvelopePdu` に変換し、`WireFrame::Envelope` として TCP writer に enqueue しなければならない（MUST）。running 状態で unconditional に `TransportError::SendFailed` を返してはならない（MUST NOT）。

#### Scenario: connected peer へ envelope frame を送る

- **GIVEN** `TcpRemoteTransport` が started である
- **AND** 対象 remote address の peer writer が登録済みである
- **AND** `OutboundEnvelope` の payload が std adapter の outbound payload codec でサポートされている
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** transport は `WireFrame::Envelope(EnvelopePdu)` を peer writer に enqueue する
- **AND** `Ok(())` を返す
- **AND** peer 側の TCP reader は同じ recipient path / sender path / priority / correlation id / payload bytes を持つ envelope frame を受信できる

#### Scenario: peer writer が無い場合は元 envelope を返す

- **GIVEN** `TcpRemoteTransport` が started である
- **AND** 対象 remote address の peer writer が未登録である
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** `Err((TransportError::ConnectionClosed, envelope))` または同等の retry-preserving error が返る
- **AND** caller は元の `OutboundEnvelope` を再 enqueue できる
- **AND** envelope は clone ではなく元値として戻る

#### Scenario: 未サポート payload は観測可能な失敗になる

- **GIVEN** `OutboundEnvelope` の `AnyMessage` payload が std adapter の outbound payload codec でサポートされていない
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** `Err((TransportError::SendFailed, envelope))` または明示的に mapping された transport error が返る
- **AND** 未サポート payload は log または test-observable error path で識別できる
- **AND** payload を empty bytes として送る silent fallback は行わない

### Requirement: outbound payload codec contract

std TCP adapter は `OutboundEnvelope` の `AnyMessage` payload を wire payload bytes に変換する契約を明示しなければならない（MUST）。少なくとも `bytes::Bytes` と `Vec<u8>` の両方をサポートしなければならない（MUST）。任意の typed payload を暗黙に serialize してはならない（MUST NOT）。

#### Scenario: サポート対象 bytes payload

- **WHEN** outbound payload が `bytes::Bytes` または `Vec<u8>` として封筒化されている
- **THEN** adapter はその bytes を `EnvelopePdu::payload` に設定する
- **AND** 受信側は同じ bytes を合意済み payload 型の `AnyMessage` として復元できる

#### Scenario: 任意 AnyMessage は暗黙 serialize されない

- **WHEN** outbound payload が serializer contract に登録されていない任意の `AnyMessage` である
- **THEN** adapter は bytes 変換を拒否する
- **AND** `Debug` 文字列や型名を payload として代用してはならない

### Requirement: connection-loss event emission

TCP client / server runtime は、remote authority が識別できる接続の read / write / decode failure または unexpected close を `RemoteEvent::ConnectionLost` として adapter 内部 sender へ通知しなければならない（MUST）。

#### Scenario: writer task failure は ConnectionLost を emit する

- **GIVEN** peer authority が `remote-sys@host:port` として識別済みである
- **WHEN** TCP writer task が write failure または channel close 以外の unexpected connection failure を観測する
- **THEN** adapter は `RemoteEvent::ConnectionLost { authority, cause, now_ms }` を remote event sender に push する
- **AND** `now_ms` は adapter-owned monotonic clock から取得する
- **AND** push 失敗は log に記録される

#### Scenario: normal shutdown は recovery を起動しない

- **WHEN** `TcpRemoteTransport::shutdown` が task を停止する
- **THEN** adapter は normal shutdown を association recovery 用 `ConnectionLost` として扱わない
- **AND** core event loop wake は `RemoteEvent::TransportShutdown` の責務に留める

### Requirement: Framed codec 統合

`transport::tcp::frame_codec` モジュールは `tokio_util::codec::{Encoder, Decoder}` を実装し、core の `Codec<T>` trait と tokio の Framed streaming を統合する SHALL。

#### Scenario: Framed の利用

- **WHEN** `transport::tcp::{client,server}` 系モジュールで `TcpStream` を Framed 化する箇所を検査する
- **THEN** `tokio_util::codec::Framed` が使われており、core の wire frame header (length+version+kind) を正しく解釈する

#### Scenario: core Codec との整合

- **WHEN** adapter の Framed decoder が受信した bytes を decode する
- **THEN** 内部で core の `Codec<T>::decode` が呼ばれ、PDU に変換される

### Requirement: `Instant::now()` の呼び出し場所の局所化

`Instant::now()` の呼び出しは adapter 側の特定箇所 (主に `handshake_driver`・`heartbeat` タイマー発火時) のみに限定される SHALL。core には `Instant::now()` を渡さず、常に monotonic millis の `u64` に変換してから core API を呼ぶ。

#### Scenario: Instant::now() の使用箇所

- **WHEN** `modules/remote-adaptor-std/src/` 配下を `Instant::now()` で grep する
- **THEN** 使用箇所は限定され、すべて monotonic millis への変換を伴う (wall clock `SystemTime::now()` は使わない)
