## MODIFIED Requirements

### Requirement: TcpRemoteTransport 型

`fraktor_remote_adaptor_std_rs::transport::tcp::TcpRemoteTransport` 型が定義され、core の `RemoteTransport` trait を実装する SHALL。TCP ベースの std remote transport として、start / shutdown / handshake / control / envelope delivery / connection-loss notification を adapter 側の送受信処理に接続する。

`TcpRemoteTransport::from_config` は `RemoteConfig` の canonical / bind / maximum frame size に加えて、inbound lane count と outbound lane count も transport 構成に反映しなければならない (MUST)。

#### Scenario: RemoteTransport trait の実装

- **WHEN** `TcpRemoteTransport` の trait 実装を検査する
- **THEN** `impl RemoteTransport for TcpRemoteTransport` が存在し、core の全メソッド (`start`, `shutdown`, `send`, `send_control`, `send_handshake`, `schedule_handshake_timeout`, `addresses`, `default_address`, `local_address_for_remote`, `quarantine`) を実装している

#### Scenario: from_config applies lane counts

- **GIVEN** `RemoteConfig::new("127.0.0.1").with_inbound_lanes(3).with_outbound_lanes(4)`
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** transport は inbound dispatch lane count と outbound writer lane count をそれぞれ設定値から構成する

### Requirement: outbound writer lanes

std TCP adapter は peer ごとに `RemoteConfig::outbound_lanes()` で指定された数の outbound writer lane を構成しなければならない (MUST)。`TcpRemoteTransport::send` は envelope から stable lane key を導出し、対象 lane の bounded writer queue へ frame を enqueue しなければならない (MUST)。

writer task は lanes を starvation なく drain し、最終的な TCP stream への write は同一 connection 内で行わなければならない (MUST)。lane queue が full の場合は `TransportError::Backpressure` または同等の retry-preserving error を返さなければならない (MUST)。

#### Scenario: outbound_lanes one keeps existing behavior

- **GIVEN** `outbound_lanes = 1`
- **WHEN** connected peer へ複数 envelope を送る
- **THEN** all frames は単一 writer lane に enqueue される
- **AND** existing single-lane ordering と同等に送信される

#### Scenario: stable lane selection

- **GIVEN** `outbound_lanes > 1`
- **AND** 同じ recipient path / sender path / correlation id を持つ envelope が複数ある
- **WHEN** `TcpRemoteTransport::send` を呼ぶ
- **THEN** それらの envelope は同じ outbound lane に enqueue される

#### Scenario: lane backpressure is observable

- **GIVEN** selected outbound lane の bounded queue が full である
- **WHEN** `TcpRemoteTransport::send(envelope)` を呼ぶ
- **THEN** retry-preserving error が返る
- **AND** caller は元 envelope を再 enqueue できる

### Requirement: inbound dispatch lanes

std TCP adapter は accepted connection または outbound client reader から得た decoded frame を、`RemoteConfig::inbound_lanes()` で指定された数の inbound dispatch lane へ振り分けなければならない (MUST)。同一 association に属する frame は同じ lane key を使わなければならない (MUST)。

各 inbound dispatch lane は `RemoteEvent::InboundFrameReceived` を remote event sender へ送る。remote event sender が closed の場合、dispatch lane は `TransportError::NotAvailable` または log で観測可能な failure を返さなければならない (MUST)。

#### Scenario: same association uses same inbound lane

- **GIVEN** inbound_lanes が 2 以上である
- **AND** 2 つの inbound frame が同じ remote authority に属する
- **WHEN** TCP reader が decoded frame を dispatch する
- **THEN** 2 つの frame は同じ inbound lane へ送られる

#### Scenario: inbound lane sender failure is observable

- **GIVEN** remote event sender が closed している
- **WHEN** inbound dispatch lane が frame を remote event sender へ送ろうとする
- **THEN** failure は log または returned error path で観測できる

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
