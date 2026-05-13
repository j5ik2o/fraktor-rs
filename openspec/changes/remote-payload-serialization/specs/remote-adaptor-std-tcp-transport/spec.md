## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: outbound envelope 配送

`TcpRemoteTransport::send` は running 状態かつ対象 peer の writer が存在する場合、`OutboundEnvelope` の `AnyMessage` payload を actor-core serialization で serialize して `EnvelopePdu` に変換し、`WireFrame::Envelope` として TCP writer に enqueue しなければならない（MUST）。running 状態で unconditional に `TransportError::SendFailed` を返してはならない（MUST NOT）。

#### Scenario: connected peer へ envelope frame を送る

- **GIVEN** `TcpRemoteTransport` が started である
- **AND** 対象 remote address の peer writer が登録済みである
- **AND** `OutboundEnvelope` の payload に対応する serializer が actor-core serialization registry に登録済みである
- **WHEN** `RemoteTransport::send(envelope)` を呼ぶ
- **THEN** transport は payload を `SerializationCallScope::Remote` で serialize する
- **AND** transport は serializer id / manifest / payload bytes を持つ `WireFrame::Envelope(EnvelopePdu)` を peer writer に enqueue する
- **AND** `Ok(())` を返す
- **AND** peer 側の TCP reader は同じ recipient path / sender path / priority / correlation id / serializer id / manifest / payload bytes を持つ envelope frame を受信できる

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
