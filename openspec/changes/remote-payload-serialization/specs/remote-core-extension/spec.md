## ADDED Requirements

### Requirement: Remote は inbound deserialization のため serialization extension を保持する

`Remote` は inbound envelope frame を local delivery 用 `InboundEnvelope` に変換するため、actor-core-kernel の `SerializationExtensionShared` または同等の concrete shared serialization handle を保持しなければならない（MUST）。この依存は actor-core-kernel の no_std 互換型に限定し、std 実行基盤型を `remote-core` に持ち込んではならない（MUST NOT）。`RemoteShared` はこの handle を別途保持せず、serialization handle を受け取った `Remote` を `RemoteShared::new(remote)` で包む薄い wrapper のままでなければならない（MUST）。

#### Scenario: Remote construction は serialization extension を受け取る

- **WHEN** `Remote` の constructor を検査する
- **THEN** inbound deserialization に使う serialization extension shared handle を受け取る経路が存在する
- **AND** `RemoteShared::new(remote)` は serialization handle を別引数で受け取らない
- **AND** `remote-core` は `std::` 型を import しない

#### Scenario: actor-core-kernel の serialization surface を使う

- **WHEN** inbound envelope payload を deserialize する実装を検査する
- **THEN** actor-core-kernel の `SerializationExtensionShared` / `SerializationExtension` 経由で deserialize する
- **AND** `EnvelopePdu` の serializer id / manifest / payload bytes から actor-core `SerializedMessage` を構築する
- **AND** remote 専用の重複 serializer registry を新設しない

## MODIFIED Requirements

### Requirement: Remote は CQS core logic 層であり Remote::run を持つ

`Remote` 構造体は CQS 原則を厳格に守る core logic 層 SHALL。状態を変更する method はすべて `&mut self`（Command）、状態を読む method は `&self`（Query）。`Remote::run(&mut self, receiver)` は排他所有時の core event loop として存在する SHALL。`Remote` 自体に共有・並行性責務を持たせてはならない（MUST NOT）。ただし inbound deserialization のため、actor-core-kernel の no_std 互換 `SerializationExtensionShared` または同等の concrete shared serialization handle を外部 extension dependency として保持してよい（MAY）。この例外は serialization registry 共有のためだけに限定し、`Remote` 自身の lifecycle / association / inbound buffer 状態を共有ロックや std 実行基盤型へ移してはならない（MUST NOT）。

#### Scenario: Remote の CQS 遵守

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** 状態を変更する method（`start` / `shutdown` / `quarantine` / `handle_remote_event` / `set_instrument` / `run` 等）はすべて `&mut self` を取る
- **AND** 状態を読む method（`addresses` / `lifecycle` / `config` 等）はすべて `&self` を取る

#### Scenario: Remote の serialization dependency は共有状態責務を増やさない

- **WHEN** `Remote` の field を検査する
- **THEN** `SerializationExtensionShared` または同等の no_std serialization shared handle を保持してよい
- **AND** lifecycle / association / inbound envelope buffer など `Remote` 所有状態を `Arc<Mutex<..>>` / `RwLock<..>` / `Cell<..>` / `RefCell<..>` / std 実行基盤型へ移してはならない
- **AND** transport port 用の `Box<dyn RemoteTransport + Send>` と instrument 用の `Box<dyn RemoteInstrument + Send>` 以外に動的ディスパッチ用の field を持たない
- **AND** `RemoteShared` は `Remote` の sharing wrapper に留まり、serialization handle を重複保持しない

#### Scenario: Remote::run の存在と所有権

- **WHEN** `impl Remote` ブロックの method 一覧を検査する
- **THEN** `pub fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a mut self, receiver: &'a mut S) -> RemoteRunFuture<'a, S>` または同等の concrete Future 型を返すシグネチャが存在する
- **AND** `self` consume ではなく `&mut self` を取る（`pub fn run(self, ..)` は存在しない）
- **AND** `async fn run` や `-> impl Future<...>` を public run API に使わない
- **AND** 内部で `Remote::handle_remote_event(event)?` と `Remote::is_terminated()` を使い、event semantics は `Remote` 内に閉じる
- **AND** `RemoteShared::run` に置き換える目的で `Remote::run` を削除してはならない（MUST NOT）

### Requirement: Codec 経路の明文化

`Remote::handle_remote_event` は inbound 側で adaptor から渡された core wire frame bytes を既存 core wire codec（`EnvelopeCodec` / `HandshakeCodec` / `ControlCodec` / `AckCodec`）で復号してから `Association` に渡す SHALL。`EnvelopePdu` の場合は、PDU に含まれる serializer id / manifest / payload bytes から actor-core `SerializedMessage` 相当を構築し、outbound 側で `SerializationCallScope::Remote` により生成された serialized payload として deserialize してから `InboundEnvelope` に buffer しなければならない（MUST）。outbound 側は現行 port 境界を維持し、`Association::next_outbound` の戻り値である `OutboundEnvelope` をそのまま `RemoteTransport::send` に渡す SHALL。core 側で `Codec<OutboundEnvelope>` / `Codec<InboundEnvelope>` を新設して raw bytes を `RemoteTransport::send` に渡してはならない（MUST NOT）。

#### Scenario: inbound decode の経路

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` を受信する
- **THEN** core wire frame header の kind に応じて `EnvelopeCodec` / `HandshakeCodec` / `ControlCodec` / `AckCodec` のいずれかで復号する
- **AND** 復号した PDU を該当 association の dispatch 経路に渡し、state transition に必要な時刻には `now_ms` を使う

#### Scenario: inbound envelope は deserialize 済み AnyMessage として buffer される

- **WHEN** `Remote::handle_remote_event` が有効な `EnvelopePdu` を復号する
- **THEN** `EnvelopePdu` の serializer id / manifest / payload bytes を使って actor-core serialization で payload を deserialize する
- **AND** buffer される `InboundEnvelope` の message は deserialize 済み payload を持つ `AnyMessage` である
- **AND** erased payload を `AnyMessage::new(Box<dyn Any + Send + Sync>)` 相当で二重 boxing してはならない
- **AND** local delivery bridge に `SerializedMessage` や raw bytes の特別扱いを要求しない

#### Scenario: inbound deserialization failure は buffer しない

- **WHEN** `EnvelopePdu` の serializer id が未登録、manifest が不正、または payload bytes が serializer で decode できない
- **THEN** `Remote::handle_remote_event` は該当 payload を `InboundEnvelope` として buffer しない
- **AND** failure は `RemotingError::CodecFailed` または serialization failure を観測できる error / log path に流れる

#### Scenario: outbound encode の経路

- **WHEN** `Remote::handle_remote_event` が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `RemoteTransport::send(envelope)` を呼ぶ
- **AND** core 側で raw bytes 化しない（wire encode は transport adaptor の責務）
