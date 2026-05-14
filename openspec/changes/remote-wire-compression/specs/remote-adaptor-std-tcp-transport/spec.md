## ADDED Requirements

### Requirement: compression table configuration と timers

`TcpRemoteTransport::from_config` は `RemoteConfig::compression_config()` を読み取り、peer ごとの actor-ref / manifest compression table と advertisement timer を構成する SHALL。timer は adapter-owned monotonic clock から `now_ms` を得て、対象 kind が enabled の場合だけ advertisement を送信しなければならない（MUST）。

#### Scenario: compression tables は config から作成される

- **GIVEN** `RemoteConfig` の actor-ref max と manifest max が `Some` である
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** transport は actor-ref table と manifest table を peer ごとに作成できる状態で初期化される

#### Scenario: disabled table は advertisement を schedule しない

- **GIVEN** `RemoteConfig` の actor-ref max が `None` である
- **WHEN** `TcpRemoteTransport` が started になる
- **THEN** actor-ref compression advertisement timer は起動しない
- **AND** manifest compression が enabled の場合は manifest advertisement timer は起動できる

#### Scenario: advertisement timer は monotonic time を使う

- **WHEN** compression advertisement timer が発火する
- **THEN** transport は adapter-owned monotonic clock から `now_ms` を計算する
- **AND** wall clock `SystemTime::now()` を使わない

### Requirement: compression control frames は transport-local に処理する

std TCP transport は inbound `ControlPdu::CompressionAdvertisement` と `ControlPdu::CompressionAck` を transport-local metadata として処理する SHALL。これらの control frame を `RemoteEvent::InboundFrameReceived` として core remote event loop へ転送してはならない（MUST NOT）。

#### Scenario: inbound advertisement は table を更新して ack を送る

- **GIVEN** TCP reader が `ControlPdu::CompressionAdvertisement` を受信する
- **WHEN** advertisement の table kind と generation が有効である
- **THEN** transport は inbound compression table を更新する
- **AND** 同じ table kind と generation を持つ `ControlPdu::CompressionAck` を peer writer へ enqueue する
- **AND** advertisement frame を `RemoteEvent::InboundFrameReceived` として送信しない

#### Scenario: inbound ack は outbound table を mark する

- **GIVEN** TCP reader が `ControlPdu::CompressionAck` を受信する
- **WHEN** ack の table kind と generation が pending advertisement と一致する
- **THEN** transport は peer の outbound compression table generation を ack 済みにする
- **AND** ack frame を `RemoteEvent::InboundFrameReceived` として送信しない

#### Scenario: invalid compression control は観測可能に失敗する

- **WHEN** TCP reader が invalid table kind または invalid entry を持つ compression control frame を受信する
- **THEN** transport は log または returned error path で failure を観測可能にする
- **AND** invalid frame を actor-level control として core へ転送しない

### Requirement: inbound compressed envelope を復元する

std TCP transport は compressed metadata を含む inbound envelope を core remote event loop へ渡す前に literal metadata へ復元する SHALL。復元に失敗した envelope は actor delivery へ進めてはならない（MUST NOT）。

#### Scenario: inbound known references は復元される

- **GIVEN** inbound actor-ref table に entry id `3` が `/user/a` として登録済みである
- **AND** inbound manifest table に entry id `5` が `example.Manifest` として登録済みである
- **WHEN** TCP reader が recipient path reference id `3` と manifest reference id `5` を持つ envelope frame を受信する
- **THEN** transport は recipient path `/user/a` と manifest `example.Manifest` を持つ literal `EnvelopePdu` に復元する
- **AND** 復元済み envelope を `RemoteEvent::InboundFrameReceived` として送信する

#### Scenario: inbound unknown reference は拒否される

- **GIVEN** inbound actor-ref table に entry id `9` が存在しない
- **WHEN** TCP reader が recipient path reference id `9` を持つ envelope frame を受信する
- **THEN** transport は decode または compression resolution failure を返す
- **AND** envelope を `RemoteEvent::InboundFrameReceived` として送信しない

## MODIFIED Requirements

### Requirement: outbound envelope 配送

`TcpRemoteTransport::send` は running 状態かつ対象 peer の writer が存在する場合、`OutboundEnvelope` の `AnyMessage` payload を actor-core serialization で serialize して `EnvelopePdu` に変換し、ack 済み compression table entry が存在する actor path / serializer manifest metadata を compressed reference として適用し、`WireFrame::Envelope` として TCP writer に enqueue しなければならない（MUST）。running 状態で unconditional に `TransportError::SendFailed` を返してはならない（MUST NOT）。

compression table に ack 済み entry が存在しない場合、transport は該当 metadata を literal として encode しなければならない（MUST）。payload bytes は compression table の対象にしてはならない（MUST NOT）。

#### Scenario: connected peer へ envelope frame を送る

- **GIVEN** `TcpRemoteTransport` が started である
- **AND** 対象 remote address の peer writer が登録済みである
- **AND** `OutboundEnvelope` の payload に対応する serializer が actor-core serialization registry に登録済みである
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** transport は payload を `SerializationCallScope::Remote` で serialize する
- **AND** transport は serializer id / manifest / payload bytes を持つ `WireFrame::Envelope(EnvelopePdu)` を peer writer に enqueue する
- **AND** `Ok(())` を返す
- **AND** peer 側の TCP reader は同じ recipient path / sender path / priority / correlation id / serializer id / manifest / payload bytes を持つ envelope frame を受信できる

#### Scenario: acked actor path and manifest は compressed references を使う

- **GIVEN** 対象 peer の actor-ref compression table に recipient path の ack 済み entry が存在する
- **AND** 対象 peer の manifest compression table に serializer manifest の ack 済み entry が存在する
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** transport は recipient path と serializer manifest を table reference metadata として encode する
- **AND** serialized payload bytes はそのまま保持する

#### Scenario: unacked metadata は literal fallback を使う

- **GIVEN** 対象 peer の compression table に recipient path の ack 済み entry が存在しない
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** transport は recipient path を literal metadata として encode する
- **AND** send は compression table miss だけを理由に失敗しない

#### Scenario: peer writer が無い場合は元 envelope を返す

- **GIVEN** `TcpRemoteTransport` が started である
- **AND** 対象 remote address の peer writer が未登録である
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** `Err((TransportError::ConnectionClosed, envelope))` または同等の retry-preserving error が返る
- **AND** caller は元の `OutboundEnvelope` を再 enqueue できる
- **AND** envelope は clone ではなく元値として戻る

#### Scenario: serializer 未登録 payload は観測可能な失敗になる

- **GIVEN** `OutboundEnvelope` の `AnyMessage` payload に対応する serializer が actor-core serialization registry に登録されていない
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** `Err((TransportError::SendFailed, envelope))` または明示的に mapping された transport error が返る
- **AND** serialization failure は log または test-observable error path で識別できる
- **AND** payload を empty bytes、debug text、型名文字列として送る silent fallback は行わない
