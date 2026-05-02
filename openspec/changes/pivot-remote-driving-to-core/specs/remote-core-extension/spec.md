## ADDED Requirements

### Requirement: RemoteEvent enum の存在

`fraktor_remote_core_rs::core::extension::RemoteEvent` enum が定義され、adapter から core への通知種別を closed enum として表現する SHALL。

#### Scenario: RemoteEvent の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event.rs` を読む
- **THEN** `pub enum RemoteEvent` が定義されている

#### Scenario: 必要なバリアントの宣言

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** 以下のバリアントを **全て** 含み、これら以外を含まない（closed enum、本 change のスコープ）
  - `InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8> }`
  - `HandshakeTimerFired { authority: TransportEndpoint, generation: u64 }`
  - `OutboundEnqueued { authority: TransportEndpoint, envelope: OutboundEnvelope }`
  - `ConnectionLost { authority: TransportEndpoint, cause: ConnectionLostCause }`
  - `TransportShutdown`

#### Scenario: 本 change スコープ外の variant

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** `OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared` 等のバリアントは含まれない（本 change で scheduling 経路が確定していないため、必要時に別 change で variant 追加と scheduling 経路を一緒に拡張する）
- **AND** これらの variant は本 change の `RemoteEvent` enum に **追加してはならない**（MUST NOT、closed enum を保ちつつ scope を絞る）

#### Scenario: open hierarchy の不在

- **WHEN** `RemoteEvent` の定義を検査する
- **THEN** `#[non_exhaustive]` および unbounded な generic は宣言されていない（closed enum として固定する）

#### Scenario: generation 型は u64

- **WHEN** `HandshakeTimerFired` バリアントの `generation` フィールド型を検査する
- **THEN** 型は `u64` であり、`HandshakeGeneration` 等の newtype でラップされていない

### Requirement: RemoteEventReceiver trait

`fraktor_remote_core_rs::core::extension::RemoteEventReceiver` trait が定義され、`Remote::run` が消費する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event_receiver.rs` を読む
- **THEN** `pub trait RemoteEventReceiver: Send` が定義されている

#### Scenario: recv のシグネチャ

- **WHEN** `RemoteEventReceiver::recv` の定義を読む
- **THEN** `fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_` または `async fn recv(&mut self) -> Option<RemoteEvent>` 形式で宣言されている

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/core/extension/` 配下の RemoteEvent / RemoteEventReceiver 関連 import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

#### Scenario: RemoteEventSink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の adapter→core push 用 trait が定義されていない（adapter 内部 sender で完結し、純増ゼロ方針を維持する）

### Requirement: Remote::run は inherent async method として駆動主導権を持つ

`Remote` 構造体に inherent method `pub async fn run<S: RemoteEventReceiver>(&mut self, receiver: &mut S) -> Result<(), RemotingError>` が定義され、event loop の主導権を core 側に集約する SHALL。`Remoting` trait に async fn を追加してはならない（MUST NOT）。`Remote` 自体に型パラメータ `<I>` を導入してはならない（MUST NOT、instrument は `Box<dyn RemoteInstrument + Send>` で保持する）。

#### Scenario: Remote::run のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `impl Remote` ブロックに `pub async fn run<S>(&mut self, receiver: &mut S) -> Result<(), RemotingError>` または同等のシグネチャが宣言されている
- **AND** `S: RemoteEventReceiver` が trait bound として要求される
- **AND** `Remote` 自体には型パラメータ `<I>` が宣言されていない

#### Scenario: Remoting trait に async fn を追加しない

- **WHEN** `Remoting` trait のメソッド一覧を検査する
- **THEN** `async fn` は存在せず、戻り値に `Future` を含まない（既存の `start` / `shutdown` / `quarantine` / `addresses` のみ）

#### Scenario: receiver 枯渇で Ok(())

- **WHEN** `RemoteEventReceiver::recv` が `None` を返す
- **THEN** `Remote::run` は `Ok(())` を返してループ終了する

#### Scenario: TransportShutdown で Ok(())

- **WHEN** receiver から `RemoteEvent::TransportShutdown` を受信する
- **THEN** `Remote::run` は `Ok(())` を返してループ終了する

#### Scenario: 復帰不能エラーで Err

- **WHEN** event 処理中に transport が永続的に失敗するなど復帰不能なエラーが発生する
- **THEN** `Remote::run` は `Err(RemotingError::TransportUnavailable)` または同等の variant を返してループ終了する
- **AND** 戻り値の `Result` を `let _ = ...` で握りつぶす経路は呼び出し側に存在しない

#### Scenario: TransportError から RemotingError への変換

- **WHEN** `Remote::run` 内で `RemoteTransport::send` / `RemoteTransport::schedule_handshake_timeout` 等が `Err(TransportError)` を返す
- **THEN** `Remote::run` は `TransportError` を `RemotingError::TransportUnavailable`（または変換ロジックで対応する `RemotingError` variant）にマップして `?` で伝播する
- **AND** `Codec::encode` / `Codec::decode` の失敗は `RemotingError::CodecFailed` 等の対応 variant にマップする
- **AND** マッピングは `Remote::run` または専用 helper で集約され、呼び出し点ごとにアドホックに `match` する経路を作らない

### Requirement: 別 Driver 型を新設しない

`Remote::run` の責務を担う `RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome` 等の新規型を core 側に追加してはならない（MUST NOT）。これらの責務は `Remote` の inherent method と既存 `Remoting` trait と `Result<(), RemotingError>` で表現する。

#### Scenario: RemoteDriver 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriver` または `pub mod driver` が定義されていない

#### Scenario: RemoteDriverHandle 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriverHandle` が定義されていない

#### Scenario: RemoteDriverOutcome enum の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub enum RemoteDriverOutcome` が定義されていない（`Result<(), RemotingError>` で「正常終了 / 異常終了」を表現する）

### Requirement: AssociationEffect::StartHandshake は Remote::run で 2 ステップ処理される

`Remote::run` のループ内で `AssociationEffect::StartHandshake { authority, timeout, generation }` を次の **2 ステップ** で処理する SHALL。adapter 側の effect application からは該当分岐を削除する。

#### Scenario: ステップ 1 — handshake request frame の送出

- **WHEN** `Remote::run` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を見つける
- **THEN** 該当の handshake request envelope を `Codec::encode` で raw bytes 化する
- **AND** 続いて既存の `RemoteTransport::send` で送出する
- **AND** `Result` を `?` で伝播する（`let _ =` で握りつぶさない）

#### Scenario: ステップ 2 — handshake timer の予約

- **WHEN** ステップ 1 の send が成功して戻る
- **THEN** `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ
- **AND** 戻り値の `Result` を `?` で伝播する
- **AND** adapter 側はこの呼出を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を内部 sender 経由で receiver に push する責務を持つ（詳細は `remote-core-transport-port` capability および `remote-adaptor-std-io-worker` capability で要件化）

#### Scenario: 順序保証

- **WHEN** ステップ 1 とステップ 2 の呼出順序を検査する
- **THEN** ステップ 1（`send`）の戻り値を確認してからステップ 2（`schedule_handshake_timeout`）を呼ぶ
- **AND** ステップ 1 が `Err` の場合、ステップ 2 は呼ばれない

### Requirement: RemoteEvent::OutboundEnqueued 処理

`Remote::run` は `RemoteEvent::OutboundEnqueued { authority, envelope }` を受信した際、該当 association に envelope を enqueue し、続けて outbound drain（next_outbound 処理）を実行する SHALL。

#### Scenario: enqueue と drain の連鎖

- **WHEN** `Remote::run` が `RemoteEvent::OutboundEnqueued { authority, envelope }` を受信する
- **THEN** `AssociationRegistry` から `authority` 対応の `Association` を取得し、`Association::enqueue(envelope)` を呼ぶ
- **AND** 続けて outbound drain helper（`next_outbound` → `Codec::encode` → `RemoteTransport::send`）を起動し、可能な限り queue を消化する

#### Scenario: 内部可変性回避

- **WHEN** adapter 側の enqueue 経路（local actor からの tell 等）を検査する
- **THEN** adapter は `AssociationRegistry` を直接 mutate せず、`RemoteEvent::OutboundEnqueued` を内部 sender に push する
- **AND** `AssociationRegistry` の所有権は `Remote` に集約されており、`Mutex` / `RwLock` / `SharedLock`（旧 `AShared` パターンの実装実体、`utils-core::SharedLock<T>`）による共有可変性が core 側に存在しない

### Requirement: Remote::run task の所有権モデル

`Remote::run` を別 task として起動する場合、`Remote` の所有権は **run task に move** されなければならない（MUST）。`Arc<Mutex<Remote>>` 等の共有可変性で `Remote` を保持してはならない（MUST NOT）。

#### Scenario: 所有権の move

- **WHEN** adapter 側 installer が `Remote::run` を tokio task として起動する経路を検査する
- **THEN** `Remote` を `Arc` / `ArcShared` / `Mutex` / `RwLock` / `SharedLock`（`utils-core::SharedLock<T>`、旧 `AShared` パターンの実装実体）にラップせず、所有権を直接 task に move する
- **AND** 起動後は `Remote` の field を外部から参照する経路が存在しない

#### Scenario: 外部制御の surface

- **WHEN** run task に対する外部制御手段を検査する
- **THEN** 以下の **2 つだけ** が存在する
  - `Sender<RemoteEvent>`（installer が clone 保持）
  - `JoinHandle<Result<(), RemotingError>>`
- **AND** これら以外（直接 method 呼出、shared state 経由）で run task の `Remote` に触れない

#### Scenario: addresses 等のクエリは installer のキャッシュから返す

- **WHEN** `Remoting::addresses()` が呼ばれる
- **THEN** installer が `transport.start()` で listening を確立した直後に `Remote::addresses()`（既存 inherent method）を呼んで保存した `Vec<Address>` キャッシュから返す
- **AND** run 中の `Remote` インスタンスにアクセスしない
- **AND** 取得経路は `Remote::addresses()` 一本に集約され、`transport.start()` の戻り値を直接キャッシュに使う経路や `Remote::start` 等の新規 API は採用しない

#### Scenario: Remoting::shutdown の停止プロトコル

- **WHEN** `Remoting::shutdown` が呼ばれる
- **THEN** installer が保持する `Sender<RemoteEvent>` 経由で `RemoteEvent::TransportShutdown` を push する
- **AND** 続けて `JoinHandle::await` で run task の終了（`Ok(())`）を待つ
- **AND** `JoinHandle` が `Err` を返した場合、`RemotingError` に変換して呼出元に伝播する

### Requirement: Codec 経路の明文化

`Remote::run` は inbound 側で raw frame を `Codec::decode` で復号してから `Association` に渡し、outbound 側で `Association::next_outbound` の戻り値を `Codec::encode` で raw bytes 化してから `RemoteTransport` に渡す SHALL。

#### Scenario: inbound decode の経路

- **WHEN** `Remote::run` が `RemoteEvent::InboundFrameReceived { authority, frame }` を受信する
- **THEN** `Codec<InboundEnvelope>::decode(&frame)` で復号する
- **AND** 復号した `InboundEnvelope` を該当 association の dispatch 経路に渡す

#### Scenario: outbound encode の経路

- **WHEN** `Remote::run` が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `Codec<OutboundEnvelope>::encode(&envelope)` で raw bytes 化する
- **AND** その raw bytes を `RemoteTransport::send` または同等の API に渡す

### Requirement: outbound watermark backpressure の発火経路

`Remote::run` は outbound enqueue / dequeue のたびに `Association::total_outbound_len()` を `RemoteConfig::outbound_high_watermark` / `outbound_low_watermark` と比較し、watermark 境界をエッジで跨いだ時にのみ `Association::apply_backpressure(BackpressureSignal::Apply)` または `Release` を発火する SHALL。境界を跨がない通常の enqueue / dequeue では発火しない。

#### Scenario: high watermark で Apply（エッジでのみ発火）

- **WHEN** `Remote::run` が outbound enqueue 直後に `Association::total_outbound_len()` を確認し、enqueue 前は `outbound_high_watermark` 以下、enqueue 後は超過になった（境界を跨いだ）
- **THEN** `Remote::run` は `Association::apply_backpressure(BackpressureSignal::Apply)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Apply, ..)` が呼ばれる
- **AND** 既に超過状態で連続 enqueue した場合、2 回目以降は `apply_backpressure` を呼ばない

#### Scenario: low watermark で Release（エッジでのみ発火）

- **WHEN** `Remote::run` が outbound dequeue 直後に `Association::total_outbound_len()` を確認し、dequeue 前は `outbound_low_watermark` 以上で Apply 状態、dequeue 後は下回った（境界を跨いだ）
- **THEN** `Remote::run` は `Association::apply_backpressure(BackpressureSignal::Release)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Release, ..)` が呼ばれる
- **AND** 既に Release 済み状態で連続 dequeue した場合、2 回目以降は `apply_backpressure` を呼ばない

#### Scenario: 設定値の経路

- **WHEN** `RemoteConfig` のフィールドを検査する
- **THEN** `pub outbound_high_watermark: usize` と `pub outbound_low_watermark: usize` が宣言され、`outbound_low_watermark < outbound_high_watermark` を validation する

### Requirement: 戻り値の握りつぶし禁止

`Remote::run` 内で `RemoteEventReceiver::recv`（戻り値 `Option`）以外の `Result` 戻り値（`RemoteTransport::*`、`Codec::*` 等）を `let _ = ...` で握りつぶしてはならない（MUST NOT）。

#### Scenario: 戻り値の明示的扱い

- **WHEN** `Remote::run` の実装ソースを検査する
- **THEN** `let _ = ...` による `Result` 握りつぶしが存在しない
- **AND** 失敗は `?` で伝播するか、`match` で観測可能な経路（log / metric / instrument）に分岐する
