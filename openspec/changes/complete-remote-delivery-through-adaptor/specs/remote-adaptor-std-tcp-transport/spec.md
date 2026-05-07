## MODIFIED Requirements

### Requirement: TcpRemoteTransport 型

`fraktor_remote_adaptor_std_rs::std::transport::tcp::TcpRemoteTransport` 型が定義され、core の `RemoteTransport` trait を実装する SHALL。TCP ベースの std remote transport として、start / shutdown / handshake / control / envelope delivery / connection-loss notification を adapter runtime に接続する。

#### Scenario: RemoteTransport trait の実装

- **WHEN** `TcpRemoteTransport` の trait 実装を検査する
- **THEN** `impl RemoteTransport for TcpRemoteTransport` が存在し、core の全メソッド (`start`, `shutdown`, `send`, `send_control`, `send_handshake`, `schedule_handshake_timeout`, `addresses`, `default_address`, `local_address_for_remote`, `quarantine`) を実装している

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

std TCP adapter は `OutboundEnvelope` の `AnyMessage` payload を wire payload bytes に変換する契約を明示しなければならない（MUST）。少なくとも `bytes::Bytes` または `Vec<u8>` のいずれかをサポートしなければならない（MUST）。任意の typed payload を暗黙に serialize してはならない（MUST NOT）。

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
