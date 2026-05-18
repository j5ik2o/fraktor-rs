# remote-adaptor-std-tcp-transport Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: TcpRemoteTransport 型

`fraktor_remote_adaptor_std_rs::transport::tcp::TcpRemoteTransport` 型が定義され、core の `RemoteTransport` trait を実装する SHALL。TCP ベースの std remote transport として、start / shutdown / handshake / control / envelope delivery / connection-loss notification を adapter 側の送受信処理に接続する。

`TcpRemoteTransport::from_config` は `RemoteConfig` の canonical / bind / maximum frame size に加えて、inbound lane count と outbound lane count も transport 構成に反映しなければならない (MUST)。

#### Scenario: 型の存在

- **WHEN** `modules/remote-adaptor-std/src/transport/tcp/base.rs` を読む
- **THEN** `pub struct TcpRemoteTransport` が定義されている

#### Scenario: RemoteTransport trait の実装

- **WHEN** `TcpRemoteTransport` の trait 実装を検査する
- **THEN** `impl RemoteTransport for TcpRemoteTransport` が存在し、core の全メソッド (`start`, `shutdown`, `send`, `send_control`, `send_handshake`, `schedule_handshake_timeout`, `addresses`, `default_address`, `local_address_for_remote`, `quarantine`) を実装している

#### Scenario: from_config applies lane counts

- **GIVEN** `RemoteConfig::new("127.0.0.1").with_inbound_lanes(3).with_outbound_lanes(4)`
- **WHEN** `TcpRemoteTransport::from_config(system_name, config)` を呼ぶ
- **THEN** transport は inbound dispatch lane count と outbound writer lane count をそれぞれ設定値から構成する

### Requirement: bind と accept loop

`TcpRemoteTransport::start` は `tokio::net::TcpListener::bind` でリスナーを開始し、accept loop の tokio task を spawn する SHALL。

#### Scenario: bind 成功後に accept loop が動作

- **WHEN** `TcpRemoteTransport::new(config)` で作成し `start()` を呼ぶ
- **THEN** 指定されたアドレスで `TcpListener` が bind され、accept loop の tokio task が生成される

#### Scenario: bind 失敗時のエラー

- **WHEN** 既に使用中のポートに対して `start()` を呼ぶ
- **THEN** `Err(TransportError::SendFailed)` または同等のエラーが返る

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
- **AND** 同じ recipient path / sender path を持つ envelope が複数ある
- **AND** それぞれの correlation id が異なる
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

std TCP adaptor は `OutboundEnvelope` の `AnyMessage` payload を actor-core serialization により wire payload bytes と serializer metadata に変換する契約を明示しなければならない（MUST）。`Vec<u8>` と `ByteString` は例外的な raw bytes fast path ではなく、登録済み serializer を通して扱わなければならない（MUST）。`bytes::Bytes` は actor-core builtin serializer 対象ではないため、caller が serializer を登録していない限り拒否しなければならない（MUST）。任意の typed payload を debug text や型名文字列へ暗黙変換してはならない（MUST NOT）。

#### Scenario: サポート対象 byte payload

- **WHEN** outbound payload が `Vec<u8>` または `ByteString` として封筒化されている
- **THEN** adaptor は actor-core serialization registry の対応 serializer で payload を serialize する
- **AND** `EnvelopePdu` には serializer id / manifest / payload bytes が設定される
- **AND** 受信側は同じ serializer metadata から元の bytes payload を `AnyMessage` として復元できる

#### Scenario: bytes::Bytes は builtin serializer なしでは拒否される

- **WHEN** outbound payload が `bytes::Bytes` として封筒化されており、caller が対応 serializer を登録していない
- **THEN** adaptor は bytes 変換を拒否する
- **AND** 旧 raw bytes fast path に fallback してはならない

#### Scenario: 登録済み typed payload は serialize される

- **WHEN** outbound payload が serializer registry に登録済みの typed `AnyMessage` である
- **THEN** adaptor は payload を `SerializationCallScope::Remote` で serialize する
- **AND** `EnvelopePdu` の payload bytes は serializer の出力であり、`Debug` 文字列や型名文字列ではない

#### Scenario: 未登録 AnyMessage は拒否される

- **WHEN** outbound payload が serializer registry に登録されていない任意の `AnyMessage` である
- **THEN** adaptor は bytes 変換を拒否する
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

### Requirement: TcpRemoteTransport は serialization extension を使う

`TcpRemoteTransport` は outbound `OutboundEnvelope` を `EnvelopePdu` に変換するため、actor-core-kernel の `SerializationExtensionShared` または同等の concrete shared serialization handle を保持しなければならない（MUST）。transport は独自 serializer registry を作ってはならず（MUST NOT）、actor system に登録された serialization extension と同じ registry を使わなければならない（MUST）。

#### Scenario: transport construction に serialization extension が接続される

- **WHEN** `RemotingExtensionInstaller` が `TcpRemoteTransport` を `Remote` に渡す
- **THEN** transport には actor system の serialization extension shared handle が設定されている
- **AND** custom setup を持つ `SerializationExtensionInstaller` が remoting installer より先に登録済みの場合はその instance を使う
- **AND** 未登録の場合は default serialization extension を登録して使う

#### Scenario: transport は独自 registry を作らない

- **WHEN** `TcpRemoteTransport` の outbound serialization 実装を検査する
- **THEN** `SerializationRegistry::from_setup` などで transport 専用 registry を新規構築しない
- **AND** actor system extension として共有される serialization surface を使う

### Requirement: TCP transport sends flush requests on targeted writer lanes

std TCP adaptor は core association が返す flush request effect を、対象 writer lane ごとの `ControlPdu::FlushRequest` frame として enqueue する SHALL。各 lane の flush request は、その lane に既に enqueue 済みの frame の後ろに置かれなければならない（MUST）。現行 TCP adaptor の writer lane は envelope を運び得る message-capable lane であり、`lane_id = 0` を control-only lane と仮定してはならない（MUST NOT）。

#### Scenario: shutdown flush targets supplied writer lanes

- **WHEN** core association が scope `Shutdown` の flush request effect を返す
- **THEN** TCP transport は effect に含まれる各 writer lane id に `ControlPdu::FlushRequest` を enqueue する
- **AND** 各 request は同じ flush id と expected ack 数を持つ
- **AND** lane ごとに異なる lane id を持つ

#### Scenario: DeathWatch flush targets all message-capable writer lanes

- **WHEN** core association が scope `BeforeDeathWatchNotification` の flush request effect を返す
- **THEN** TCP transport は effect に含まれる message-capable writer lane id すべてに `ControlPdu::FlushRequest` を enqueue する
- **AND** 現行 TCP adaptor では lane `0` も envelope を運び得るため、lane `0` を自動除外しない

#### Scenario: dedicated control-only lane is optional and excluded from DeathWatch scope

- **GIVEN** 将来の TCP 実装が envelope を運ばない dedicated control-only lane を追加している
- **WHEN** scope `BeforeDeathWatchNotification` の対象 lane set を構築する
- **THEN** dedicated control-only lane は対象から外してよい
- **AND** envelope を運び得る writer lane はすべて対象に含める

#### Scenario: lane backpressure is observable

- **GIVEN** flush request を enqueue すべき対象 lane の queue が full である
- **WHEN** TCP transport が flush request effect を実行する
- **THEN** failure は log または returned error path で観測できる
- **AND** association flush session は timeout または failure outcome へ進められる

### Requirement: TCP inbound dispatch routes flush control frames to core

std TCP adaptor は inbound `ControlPdu::FlushRequest` / `ControlPdu::FlushAck` を actor-core delivery へ渡さず、`RemoteEvent::InboundFrameReceived` または同等の core event として `Remote::handle_remote_event` へ渡す SHALL。flush request への ack 生成は、現行の heartbeat response と同じく core control-PDU handling が `RemoteTransport` 経由で行う。

#### Scenario: inbound flush request is routed as control frame

- **WHEN** TCP inbound lane が `ControlPdu::FlushRequest { flush_id, lane_id, expected_acks, .. }` を受信する
- **THEN** adaptor は `WireFrame::Control(ControlPdu::FlushRequest { .. })` を `RemoteEvent::InboundFrameReceived` として core event loop へ渡す
- **AND** flush request を actor-core envelope delivery へ進めない

#### Scenario: inbound flush ack is routed to core

- **WHEN** TCP inbound lane が `ControlPdu::FlushAck` を受信する
- **THEN** adaptor は `RemoteEvent::InboundFrameReceived` または同等の core event として ack を `Remote::handle_remote_event` へ渡す
- **AND** core association が flush session state を更新できる

#### Scenario: flush ack send failure is observable

- **WHEN** core が inbound flush request に対する flush ack を `RemoteTransport` 経由で送信し、transport が失敗を返す
- **THEN** failure は log または returned error path に残す
- **AND** failure を silent drop しない

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

#### Scenario: local outbound disabled でも inbound advertisement は ack する

- **GIVEN** `RemoteConfig` の actor-ref max が `None` である
- **WHEN** TCP reader が actor-ref の `ControlPdu::CompressionAdvertisement` を受信する
- **THEN** transport は inbound actor-ref table を更新する
- **AND** 同じ table kind と generation を持つ `ControlPdu::CompressionAck` を peer writer へ enqueue する
- **AND** 後続の inbound envelope に含まれる actor-ref table reference を復元できる

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
