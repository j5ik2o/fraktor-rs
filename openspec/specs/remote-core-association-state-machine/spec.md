# remote-core-association-state-machine Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: Association 型の存在

`fraktor_remote_core_rs::domain::association::Association` 型が定義され、per-remote-node の状態と send queue を集約する SHALL。Pekko `Association` (Scala, 1240行) に対応する。

#### Scenario: Association struct の存在

- **WHEN** `modules/remote-core/src/association/` を検査する
- **THEN** `pub struct Association` が定義されている

#### Scenario: 状態と send queue の集約

- **WHEN** `Association` 型のフィールドを検査する
- **THEN** `state` (associate state machine) と `send_queue` (priority queue) が同じ型に集約されており、別々の型 (例: `EndpointWriter` と `EndpointAssociationCoordinator`) に分散していない

### Requirement: AssociationState 状態列挙

`fraktor_remote_core_rs::domain::association::AssociationState` enum が定義され、`Idle`、`Handshaking`、`Active`、`Gated`、`Quarantined` の各バリアントを持つ SHALL。

#### Scenario: AssociationState の存在

- **WHEN** `modules/remote-core/src/association/association_state.rs` を読む
- **THEN** `pub enum AssociationState` が定義され、`Idle`、`Handshaking { endpoint, started_at }`、`Active { remote_node, established_at }`、`Gated { resume_at }`、`Quarantined { reason, resume_at }` のバリアントを含む

#### Scenario: Active 状態の判定

- **WHEN** `AssociationState::Active { .. }` のインスタンスに対し `is_active()` メソッドを呼ぶ
- **THEN** `true` が返る

### Requirement: &mut self ベースの状態遷移

`Association` の状態遷移メソッドはすべて `&mut self` を取り、内部可変性 (`SpinSyncMutex<T>` + `&self`) を使わない SHALL。

#### Scenario: associate メソッド

- **WHEN** `Association::associate` の定義を読む
- **THEN** `fn associate(&mut self, endpoint: TransportEndpoint, now_ms: u64 /* monotonic millis */) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている (戻り値は必ず連続コンテナ型)

#### Scenario: handshake_accepted メソッド

- **WHEN** `Association::handshake_accepted` の定義を読む
- **THEN** `fn handshake_accepted(&mut self, remote_node: RemoteNodeId, now_ms: u64 /* monotonic millis */) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: handshake_timed_out メソッド

- **WHEN** `Association::handshake_timed_out` の定義を読む
- **THEN** `fn handshake_timed_out(&mut self, now_ms: u64 /* monotonic millis */, resume_at_ms: Option<u64 /* monotonic millis */>) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: quarantine メソッド

- **WHEN** `Association::quarantine` の定義を読む
- **THEN** `fn quarantine(&mut self, reason: QuarantineReason, now_ms: u64 /* monotonic millis */) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: gate メソッド

- **WHEN** `Association::gate` の定義を読む
- **THEN** `fn gate(&mut self, resume_at_ms: Option<u64 /* monotonic millis */>, now_ms: u64 /* monotonic millis */) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: recover メソッド

- **WHEN** `Association::recover` の定義を読む
- **THEN** `fn recover(&mut self, endpoint: Option<TransportEndpoint>, now_ms: u64 /* monotonic millis */) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: 内部可変性の不在

- **WHEN** `Association` 型のフィールドを検査する
- **THEN** どのフィールドも `Cell<T>`・`RefCell<T>`・`SpinSyncMutex<T>`・`AShared<T>` を含まない (これらは adapter 側で wrap する)

### Requirement: 時刻入力の型と単位 (monotonic millis)

`Association` の状態遷移メソッドはすべて時刻を **monotonic millis** として受け取り、`Instant::now()` を内部で呼ばない SHALL。`now` パラメータは wall clock (epoch 経過時間) ではなく、プロセス起動からの単調増加 millis (adapter 側で `Instant::now()` の差分を渡す) を表現する。**本 change では専用 newtype は導入せず、`u64` に rustdoc/comment で monotonic millis であることを明記する。**

#### Scenario: monotonic millis であることの明示

- **WHEN** `Association::associate`・`handshake_accepted`・`handshake_timed_out`・`quarantine`・`gate`・`recover`・`enqueue` 等の `now` 引数を検査する
- **THEN** doc comment または引数名/comment (`now_ms: u64 /* monotonic millis */`) で **monotonic millis** であることが明示されており、wall clock millis とは区別されている

#### Scenario: Instant 直接呼び出しの不在

- **WHEN** `modules/remote-core/src/association/` 配下のすべての `.rs` ファイルを検査する
- **THEN** `Instant::now()`・`SystemTime::now()`・`std::time::` の呼び出しが存在しない

#### Scenario: handshake_timed_out での時刻比較

- **WHEN** `handshake_timed_out` 実装が `started_at` と `now` を比較する
- **THEN** 両方が monotonic millis (同一時刻ソース) として扱われ、wall clock jump による誤判定が起きない

### Requirement: AssociationEffect 出力型

`fraktor_remote_core_rs::domain::association::AssociationEffect` enum が定義され、状態遷移の副作用 (送信、ハンドシェイク開始、ライフサイクルイベント発火、quarantine 通知等) を表現する SHALL。

#### Scenario: AssociationEffect の存在

- **WHEN** `modules/remote-core/src/association/association_effect.rs` または同等の場所を読む
- **THEN** `pub enum AssociationEffect` が定義され、`StartHandshake { endpoint }`、`SendEnvelopes { envelopes }`、`DiscardEnvelopes { reason, envelopes }`、`PublishLifecycle(fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent)` 等のバリアントを含む

#### Scenario: 状態遷移メソッドの戻り値は連続コンテナ

- **WHEN** `Association::associate`・`handshake_accepted`・`handshake_timed_out`・`quarantine`・`gate`・`recover`・`enqueue` のいずれかの戻り値型を読む
- **THEN** `Vec<AssociationEffect>`、`SmallVec<[AssociationEffect; N]>`、`tinyvec::ArrayVec<[AssociationEffect; N]>` 等の **連続コンテナ型** を返す (単一 `AssociationEffect` は禁止。複数 effect の同時出力 — 例: state 遷移 + deferred flush + lifecycle publish — が必要なため)

#### Scenario: 単一 AssociationEffect 戻り値の不在

- **WHEN** `Association` の状態遷移メソッドの戻り値型を検査する
- **THEN** `AssociationEffect` 単独を返すメソッドは存在しない (必ずコンテナ経由)

### Requirement: Association の送信経路 API

`Association` は送信経路の公開 API として `enqueue`・`next_outbound`・`apply_backpressure` を持ち、それぞれが Pekko `Association` の対応機能を提供する SHALL。これらは内部の `SendQueue`・deferred queue・状態機械を協調制御する。

#### Scenario: enqueue メソッドのシグネチャ

- **WHEN** `Association::enqueue` の定義を読む
- **THEN** `fn enqueue(&mut self, envelope: OutboundEnvelope) -> Vec<AssociationEffect>` または同等のシグネチャが宣言されている

#### Scenario: next_outbound メソッドのシグネチャ

- **WHEN** `Association::next_outbound` の定義を読む
- **THEN** `fn next_outbound(&mut self) -> Option<OutboundEnvelope>` が宣言されている

#### Scenario: apply_backpressure メソッドのシグネチャ

- **WHEN** `Association::apply_backpressure` の定義を読む
- **THEN** `fn apply_backpressure(&mut self, signal: BackpressureSignal)` が宣言されている

### Requirement: enqueue の状態別セマンティクス

`Association::enqueue` は現在の状態に応じて以下の副作用を返す SHALL。

#### Scenario: Active 状態で enqueue

- **WHEN** `Active` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** envelope は内部 `SendQueue` に offer され、戻り値は空の `Vec<AssociationEffect>` (`next_outbound` で後から取り出す)

#### Scenario: Handshaking 状態で enqueue

- **WHEN** `Handshaking` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** envelope は deferred queue に蓄積され、戻り値は空の `Vec<AssociationEffect>` (`handshake_accepted` 時に `SendEnvelopes` effect としてまとめて flush される)

#### Scenario: Quarantined 状態で enqueue

- **WHEN** `Quarantined { reason, .. }` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** 戻り値に `AssociationEffect::DiscardEnvelopes { reason, envelopes: vec![envelope] }` が含まれる (即破棄)

#### Scenario: Gated 状態で enqueue

- **WHEN** `Gated { resume_at, .. }` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** envelope は deferred queue に蓄積され、戻り値は空の `Vec<AssociationEffect>` (recover 時に flush または discard される)

#### Scenario: Idle 状態で enqueue

- **WHEN** `Idle` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** envelope は deferred queue に蓄積され、戻り値は空の `Vec<AssociationEffect>` (`associate` 後の handshake 完了時に flush される)

### Requirement: SendQueue priority logic

`Association` 内の `SendQueue` は system priority、normal user、large-message user の queue を持ち、system priority を最優先で取り出す SHALL。normal user と large-message user はどちらも wire 上は user message であり、large-message queue は送信側の local scheduling と capacity 分離のために使われる。

`Association::next_outbound` と `Association::apply_backpressure` は内部の `SendQueue` に委譲することで、この priority / lane ロジックを公開する。

#### Scenario: system queue の優先

- **WHEN** system priority、normal user、large-message user のすべてに message があり、`Association::next_outbound()` を呼ぶ
- **THEN** system priority のメッセージが先に返される

#### Scenario: normal user は large-message user より先に drain される

- **WHEN** normal user と large-message user の両方の message が queue にあり、system priority message がない
- **THEN** normal user message が large-message user message より先に返される

#### Scenario: user queue の backpressure pause

- **WHEN** `Association` に `apply_backpressure(BackpressureSignal::Apply)` を適用してから `next_outbound()` を呼ぶ
- **THEN** normal user と large-message user の message は取り出されない
- **AND** system priority message は取り出される

#### Scenario: backpressure release

- **WHEN** `apply_backpressure(BackpressureSignal::Release)` を適用してから `next_outbound()` を呼ぶ
- **THEN** normal user と large-message user の message も取り出される

### Requirement: Association は large-message destination settings を enqueue に反映する

`Association::from_config` は `RemoteConfig::large_message_destinations()` と `RemoteConfig::outbound_large_message_queue_size()` を使って large-message enqueue policy を構成しなければならない (MUST)。

`Association::enqueue` は `OutboundPriority::User` の envelope について recipient absolute path が configured large-message destination pattern に一致する場合、normal user queue ではなく large-message queue に offer しなければならない (MUST)。`OutboundPriority::System` の envelope は pattern に一致しても system queue に入らなければならない (MUST)。

#### Scenario: matching user recipient は large-message queue に入る

- **GIVEN** `RemoteConfig` に `/user/large-*` の large-message destination pattern が設定されている
- **AND** `Association` がその config から作られている
- **WHEN** recipient path `/user/large-worker` の user envelope を enqueue する
- **THEN** envelope は large-message queue に入る
- **AND** normal user queue capacity は消費しない

#### Scenario: non-matching user recipient は normal user queue に入る

- **GIVEN** `RemoteConfig` に `/user/large-*` の large-message destination pattern が設定されている
- **WHEN** recipient path `/user/small-worker` の user envelope を enqueue する
- **THEN** envelope は normal user queue に入る
- **AND** large-message queue capacity は消費しない

#### Scenario: system envelope は large-message pattern より優先される

- **GIVEN** large-message destination pattern に一致する recipient path を持つ system envelope
- **WHEN** `Association::enqueue` を呼ぶ
- **THEN** envelope は system queue に入る
- **AND** large-message queue capacity は消費しない

#### Scenario: large-message queue capacity は config から来る

- **GIVEN** `RemoteConfig::with_outbound_large_message_queue_size(1)` で作られた `Association`
- **WHEN** matching user envelope を 2 件 enqueue する
- **THEN** 1 件目は accepted になる
- **AND** 2 件目は元 envelope を保持した queue-full outcome になる

### Requirement: QuarantineReason 型

`fraktor_remote_core_rs::domain::association::QuarantineReason` 型が定義され、quarantine の理由を保持する SHALL。

#### Scenario: QuarantineReason の存在

- **WHEN** `modules/remote-core/src/association/quarantine_reason.rs` を読む
- **THEN** `pub struct QuarantineReason` が定義され、`new(message: impl Into<String>)` コンストラクタと `message(&self) -> &str` accessor を持つ

### Requirement: handshake 状態の管理

`Association` は handshake 進行中の状態 (`Handshaking { endpoint, started_at }`) を保持し、`handshake_accepted`・`handshake_timed_out` メソッドで遷移する SHALL。

#### Scenario: handshake_timed_out で Gated へ

- **WHEN** `Handshaking` 状態の `Association` に `handshake_timed_out(now)` を呼ぶ
- **THEN** 状態が `Gated { resume_at: Some(_) }` に遷移する

### Requirement: deferred envelope queue

`Association` は handshake 完了前に到着した outbound envelope を保持する deferred queue を持ち、handshake 完了後にまとめて送信する SHALL。

#### Scenario: handshake 中の deferred 蓄積

- **WHEN** `Handshaking` 状態の `Association` に `enqueue(envelope)` を呼ぶ
- **THEN** envelope は deferred queue に蓄積され、戻り値の `Vec<AssociationEffect>` は空 (`AssociationEffect::SendEnvelopes` は返らない)

#### Scenario: handshake 完了で deferred を flush

- **WHEN** deferred queue に envelope が積まれた状態で `handshake_accepted(remote_node, now)` を呼ぶ
- **THEN** 戻り値の effect 列に `AssociationEffect::SendEnvelopes { envelopes }` が含まれ、deferred 内容を flush する

#### Scenario: quarantine で deferred を破棄

- **WHEN** deferred queue に envelope が積まれた状態で `quarantine(reason, now)` を呼ぶ
- **THEN** 戻り値の effect 列に `AssociationEffect::DiscardEnvelopes { reason, envelopes }` が含まれる

### Requirement: recover 状態遷移

`Association::recover` メソッドは `Gated` または `Quarantined` 状態から、endpoint 指定の有無に応じて `Handshaking` または `Idle` へ遷移する SHALL。`Idle`・`Handshaking`・`Active` 状態での `recover` 呼び出しは no-op または `InvalidTransition` とする。

#### Scenario: recover(Some(endpoint)) で Gated から Handshaking へ

- **WHEN** `Gated { resume_at }` 状態の `Association` に `recover(Some(endpoint), now)` を呼ぶ
- **THEN** 内部状態が `Handshaking { endpoint, started_at: now }` に遷移し、戻り値の effect 列に `AssociationEffect::StartHandshake { endpoint }` が含まれる

#### Scenario: recover(Some(endpoint)) で Quarantined から Handshaking へ

- **WHEN** `Quarantined { reason, resume_at }` 状態の `Association` に `recover(Some(endpoint), now)` を呼ぶ
- **THEN** 内部状態が `Handshaking { endpoint, started_at: now }` に遷移し、戻り値の effect 列に `AssociationEffect::StartHandshake { endpoint }` が含まれる

#### Scenario: recover(None) で Idle へ

- **WHEN** `Gated` 状態の `Association` に `recover(None, now)` を呼ぶ
- **THEN** 内部状態が `Idle` に遷移し、`AssociationEffect::StartHandshake` は戻り値に含まれない

#### Scenario: Active 状態からの recover は no-op

- **WHEN** `Active` 状態の `Association` に `recover(Some(endpoint), now)` を呼ぶ
- **THEN** 内部状態は `Active` のままで、戻り値は空の effect 列である

#### Scenario: Idle 状態からの recover は no-op

- **WHEN** `Idle` 状態の `Association` に `recover(Some(endpoint), now)` または `recover(None, now)` を呼ぶ
- **THEN** 内部状態は `Idle` のままで、戻り値は空の effect 列である (Idle からは `associate` で遷移すべきであり `recover` は無効)

#### Scenario: Handshaking 状態からの recover は no-op

- **WHEN** `Handshaking` 状態の `Association` に `recover(Some(endpoint), now)` または `recover(None, now)` を呼ぶ
- **THEN** 内部状態は `Handshaking` のままで、戻り値は空の effect 列である (Handshaking 中は `handshake_accepted` または `handshake_timed_out` で自然遷移すべき)

### Requirement: Association は instrument hook を呼び出す

`Association` は状態遷移および送受信のキー点で `RemoteInstrument` の対応 method を呼び出さなければならない（MUST）。instrument 参照は引数として受け取る（field として保持しない）。

#### Scenario: associate / handshake_accepted で record_handshake 発火

- **WHEN** `Association::associate` または `handshake_accepted` が呼ばれる
- **THEN** 同一呼出の中で対応する `RemoteInstrument::record_handshake(authority, phase, now_ms)` が呼ばれる
- **AND** phase は `Started` / `Accepted` / `Rejected` のいずれかを取る

#### Scenario: handshake_timed_out で record_handshake(Rejected)

- **WHEN** `Association::handshake_timed_out` が呼ばれる
- **THEN** `RemoteInstrument::record_handshake(authority, HandshakePhase::Rejected, now_ms)` が呼ばれる

#### Scenario: quarantine で record_quarantine

- **WHEN** `Association::quarantine(reason, now_ms)` が呼ばれる
- **THEN** `RemoteInstrument::record_quarantine(authority, reason, now_ms)` が呼ばれる

#### Scenario: enqueue / next_outbound で on_send 発火

- **WHEN** `Association::next_outbound` が `Some(envelope)` を返す
- **THEN** 同一呼出または直後の `Remote::handle_remote_event` 経路で `RemoteInstrument::on_send(&envelope)` が呼ばれる
- **AND** 呼び出し点は Association 内部または `Remote::handle_remote_event` の outbound 駆動経路のいずれかで明文化される

#### Scenario: inbound dispatch で on_receive 発火

- **WHEN** `Remote::handle_remote_event` が inbound core wire frame を decode し、復元した inbound envelope 相当の値を Association に渡す
- **THEN** `RemoteInstrument::on_receive(&envelope)` が呼ばれる

#### Scenario: apply_backpressure で record_backpressure

- **WHEN** `Association::apply_backpressure(signal)` が呼ばれる
- **THEN** `RemoteInstrument::record_backpressure(authority, signal, correlation_id, now_ms)` が呼ばれる
- **AND** correlation_id は backpressure 文脈で観測可能な情報がある場合に限り `Some(_)` を取り、無ければ `None`

### Requirement: instrument 引数の渡し方

`Association` の状態遷移メソッドおよび送受信メソッドは `&mut dyn RemoteInstrument` を引数で受け取り、`Association` 自身が instrument を field として所有してはならない（MUST NOT）。型パラメータ `<I: RemoteInstrument>` を `Association` メソッドに導入してはならない（MUST NOT）。正式リリース前の破壊的変更を許容し、最終形では instrument を通らない公開 mutation API と `*_with_instrument` 併設 API を残さない。

#### Scenario: instrument を field 保持しない

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `RemoteInstrument` を直接または間接的に保持しない
- **AND** instrument 参照は呼び出し時に外部（`Remote::handle_remote_event`）から渡される

#### Scenario: hook 系メソッドの引数

- **WHEN** `Association::associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `apply_backpressure` の最終シグネチャを検査する
- **THEN** いずれも `instrument: &mut dyn RemoteInstrument` を引数として受け取る
- **AND** 呼び出し側（`Remote::handle_remote_event`）は `&mut *self.instrument`（`self.instrument: Box<dyn RemoteInstrument + Send>` から `DerefMut` 経由）で参照を取得する
- **AND** メソッドシグネチャに型パラメータ `<I>` が出現しない
- **AND** 同じ責務を持つ `*_with_instrument` 併設 API と instrument 無し API が同時に公開されていない

#### Scenario: enqueue / next_outbound のシグネチャ

- **WHEN** `Association::enqueue` および `Association::next_outbound` のシグネチャを検査する
- **THEN** `Association::enqueue(envelope)` は instrument 引数を取らない（純粋な queue 投入のみで I/O を伴わないため）
- **AND** `on_send` の発火は `Association::next_outbound` の戻り値経路（または `Remote::handle_remote_event` の outbound drain helper）で行い、その時点で `&mut dyn RemoteInstrument` を渡すか戻り値経由で発火する
- **AND** 「enqueue で on_send」「next_outbound で on_send」の二重発火が起きない

### Requirement: outbound queue の総長クエリ

`Association` は outbound queue（`SendQueue` の system + user）の合計長を返すクエリメソッドを提供する SHALL。これは `Remote::handle_remote_event` が watermark backpressure を制御するために使用する。

#### Scenario: total_outbound_len のシグネチャ

- **WHEN** `Association::total_outbound_len` または同等の query method の定義を読む
- **THEN** `fn total_outbound_len(&self) -> usize` が宣言されている（CQS 準拠で `&self`）
- **AND** 戻り値は system priority queue と user priority queue の合計長を表す

#### Scenario: deferred queue は含めない

- **WHEN** `Handshaking` 状態で deferred queue に envelope が積まれている
- **THEN** `total_outbound_len()` は `SendQueue` のみの長さを返し、deferred queue を含めない（deferred は handshake 完了で flush される一時バッファであり、watermark 判定の対象外）

### Requirement: watermark 連動の自動 backpressure 発火経路

`Remote::handle_remote_event` は outbound enqueue / dequeue のたびに `Association::total_outbound_len()` を `outbound_high_watermark` / `outbound_low_watermark` と比較し、watermark 境界をエッジで跨いだ時にのみ `Association::apply_backpressure` を呼び出す SHALL。high watermark の signal は、internal drain を止めない意味と一致していなければならない（MUST）。

#### Scenario: BackpressureSignal variant の仕様は実装と一致する

- **WHEN** `BackpressureSignal` enum の variant を検査する
- **THEN** live OpenSpec は実装に存在する variant だけを仕様化する
- **AND** `Notify` variant が存在する場合、その意味は「internal high watermark を跨いだことの通知であり、user lane pause はしない」として明記されている
- **AND** `Apply` variant は「user lane を pause する」意味として残る

#### Scenario: Notify は user lane を pause しない

- **GIVEN** `BackpressureSignal::Notify` が high watermark 用に採用されている
- **WHEN** `Association::apply_backpressure(Notify, ...)` を呼ぶ
- **THEN** `SendQueue` の user lane は paused にならない
- **AND** instrumentation には high watermark crossing が記録される

#### Scenario: Apply は明示的な pause を表す

- **WHEN** adapter / upper layer が明示的な backpressure として `BackpressureSignal::Apply` を呼ぶ
- **THEN** user lane は paused になる
- **AND** `BackpressureSignal::Release` で resume する

#### Scenario: backpressure state は Association が保持する

- **WHEN** 同じ signal を 2 回連続で発火する
- **THEN** `Association` は idempotent に動作し、2 回目は state 遷移を伴わない
- **AND** instrument の `record_backpressure` は state 変化を伴った発火点でのみ呼ばれる（または instrument 側で重複を吸収する）

### Requirement: AssociationEffect::StartHandshake は Remote::handle_remote_event で実行される（adapter 無視を禁止）

`Association::recover` および `associate` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を出力した場合、その effect は `Remote::handle_remote_event` の経路で `RemoteTransport` 経由の handshake 開始に dispatch されなければならない（MUST）。adapter 側で `StartHandshake` を ignore する分岐を持ってはならない（MUST NOT）。

#### Scenario: Remote::handle_remote_event による StartHandshake 実行（2 ステップ）

- **WHEN** `Association::recover(Some(endpoint), now)` または `associate(...)` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を返す
- **THEN** `Remote::handle_remote_event` は同一 effect 列処理の中で次の 2 ステップを順に実行する
  1. `HandshakePdu::Req(HandshakeReq::new(local, remote))` を構築し、`RemoteTransport::send_handshake` で送出する
  2. 続けて `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)`（`remote-core-transport-port` capability で要件化）を呼ぶ
- **AND** ステップ 1 が `Err` の場合、ステップ 2 は呼ばれない
- **AND** adapter 側は `schedule_handshake_timeout` 呼出を契機に tokio task で sleep を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }` を adapter 内部 sender 経由で receiver に push する

#### Scenario: adapter 側の StartHandshake 無視分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/effect_application.rs` の dispatch を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } => /* ignore */` または同等の no-op 分岐が存在しない

### Requirement: handshake generation の管理（u64 inline）

`Association` は handshake ごとに単調増加する generation 値を `u64` フィールドとして保持し、`AssociationEffect::StartHandshake` および `RemoteEvent::HandshakeTimerFired` で同じ `u64` を参照することで、古い timeout の発火を無視する SHALL。`HandshakeGeneration` 等の newtype は新設してはならない（MUST NOT、純増ゼロ方針）。

#### Scenario: generation の保持

- **WHEN** `Association` 構造体のフィールドを検査する
- **THEN** `handshake_generation: u64` が保持され、`Handshaking` 状態に入るたびに `wrapping_add(1)` で +1 される
- **AND** `HandshakeGeneration` newtype や `pub struct HandshakeGeneration(u64)` が定義されていない

#### Scenario: 古い timeout の無視

- **WHEN** `Remote::handle_remote_event` が `RemoteEvent::HandshakeTimerFired { authority, generation: g_event }` を受信し、現在の `Association` の generation が `g_current` であって `g_current != g_event` である
- **THEN** `Remote::handle_remote_event` は `Association::handshake_timed_out` を呼ばず、event を破棄する
- **AND** 破棄は instrument の `record_handshake` を発火しない（古いイベントなので観測対象外）
- **AND** 比較演算子は `!=` を使用する（`>` は使用しない。`wrapping_add` で +1 を続けると `u64::MAX → 0` の wrap 時に `g_current > g_event` が成立せず stale 判定が漏れるため）

#### Scenario: AssociationEffect::StartHandshake の generation フィールド

- **WHEN** `AssociationEffect::StartHandshake` の variant 定義を検査する
- **THEN** `StartHandshake { authority: TransportEndpoint, timeout: core::time::Duration, generation: u64 }` または同等のフィールド構成を持つ
- **AND** generation の型は `u64` であり、newtype でラップされていない

### Requirement: system message redelivery state

`Association` は system priority envelope の ACK/NACK redelivery state を所有する SHALL。対象は remote DeathWatch に必要な `Watch`、`Unwatch`、`DeathWatchNotification` 系 system message であり、user priority envelope はこの state に保持してはならない（MUST NOT）。

#### Scenario: system envelope receives sequence number

- **WHEN** `Association::enqueue` に system priority envelope が渡される
- **THEN** association は per-remote-node の単調増加 sequence number を割り当てる
- **AND** envelope は ACK を受けるまで resend window に保持される

#### Scenario: user envelope is not tracked by redelivery state

- **WHEN** `Association::enqueue` に user priority envelope が渡される
- **THEN** association は redelivery sequence number を割り当てない
- **AND** user envelope は ACK/NACK resend window に保持されない

#### Scenario: cumulative ack removes pending envelopes

- **GIVEN** sequence number `10`、`11`、`12` の system envelope が pending である
- **WHEN** `AckPdu { cumulative_ack: 11, .. }` を association に適用する
- **THEN** sequence number `10` と `11` は pending から削除される
- **AND** sequence number `12` は pending に残る

#### Scenario: nack bitmap selects missing envelopes for resend

- **GIVEN** sequence number `20` から `23` の system envelope が pending である
- **WHEN** `AckPdu { cumulative_ack: 20, nack_bitmap }` が sequence number `22` の欠落を示す
- **THEN** association は sequence number `22` の envelope を resend effect に含める
- **AND** ACK 済みの sequence number `20` は resend effect に含めない

### Requirement: inbound system sequence tracking

`Association` は inbound system priority envelope の sequence number を tracking し、受信済み範囲から cumulative ACK と NACK bitmap を生成する SHALL。重複 sequence number は actor-core へ二重配送してはならない（MUST NOT）。

#### Scenario: in-order system envelope advances ack

- **GIVEN** inbound cumulative ACK が `40` である
- **WHEN** sequence number `41` の system envelope を受信する
- **THEN** inbound cumulative ACK は `41` へ進む
- **AND** association は `AckPdu` 送信 effect を返す

#### Scenario: gap produces nack bitmap

- **GIVEN** inbound cumulative ACK が `50` である
- **WHEN** sequence number `52` の system envelope を受信する
- **THEN** inbound cumulative ACK は `50` のまま維持される
- **AND** association は sequence number `51` の欠落を示す NACK bitmap を持つ `AckPdu` 送信 effect を返す

#### Scenario: duplicate inbound system envelope is ignored

- **GIVEN** sequence number `60` の system envelope がすでに actor-core へ配送済みである
- **WHEN** 同じ sequence number `60` の system envelope を再受信する
- **THEN** association は actor-core delivery 対象として返さない
- **AND** ACK 状態は再送元が停止できる形で返される

### Requirement: association flush session state

`Association` は shutdown flush と DeathWatch notification 前 flush の session state を所有する SHALL。session state は flush id、flush scope、caller が渡した対象 writer lane id 集合、期待 ack 数、ack 済み lane id 集合、deadline monotonic millis、完了状態を保持しなければならない（MUST）。timer、async wait、TCP の lane topology 推定は保持してはならない（MUST NOT）。

#### Scenario: shutdown flush starts session for caller supplied lanes

- **GIVEN** caller が shutdown flush の対象 writer lane id として `[0, 1, 2]` を渡す
- **WHEN** active association に shutdown flush を開始する
- **THEN** association は新しい flush id を割り当てる
- **AND** lane `0`、`1`、`2` を対象にした flush request effect を返す
- **AND** expected ack 数は `3` になる
- **AND** deadline は caller から渡された monotonic millis と flush timeout から計算される

#### Scenario: DeathWatch flush uses caller supplied message-capable lanes

- **GIVEN** caller が DeathWatch notification 前 flush の対象 writer lane id として `[0, 1]` を渡す
- **WHEN** active association に DeathWatch notification 前 flush を開始する
- **THEN** association は lane `0`、`1` を対象にした flush request effect を返す
- **AND** association は `lane_id = 0` を control-only lane と仮定して除外しない

#### Scenario: empty lane set completes immediately

- **WHEN** caller が空の対象 writer lane id 集合で flush を開始する
- **THEN** association は flush request effect を返さない
- **AND** flush completed effect を即時に返す

#### Scenario: flush ack completes session

- **GIVEN** flush id `10` の session が lane `0` と lane `1` の ack を待っている
- **WHEN** lane `0` と lane `1` の `FlushAck` を association に適用する
- **THEN** association は flush completed effect を返す
- **AND** session は active flush map から削除される

#### Scenario: duplicate flush ack is ignored

- **GIVEN** flush id `10` の session が lane `0` の ack をすでに観測している
- **WHEN** lane `0` の `FlushAck` を再度 association に適用する
- **THEN** remaining ack count は減らない
- **AND** duplicate ack だけでは flush completed effect を返さない

#### Scenario: flush timeout releases session

- **GIVEN** flush id `10` の session が lane `1` の ack を待っている
- **WHEN** monotonic millis が session deadline 以上になった timer input を association に適用する
- **THEN** association は flush timed-out effect を返す
- **AND** session は active flush map から削除される

#### Scenario: connection loss fails pending flush

- **GIVEN** active association に pending flush session がある
- **WHEN** connection lost または quarantine transition が association に適用される
- **THEN** association は pending flush session を failed または timed-out outcome として完了させる effect を返す
- **AND** caller が shutdown または DeathWatch notification の後続処理へ進める

#### Scenario: flush does not start while prior outbound queue is still pending

- **GIVEN** flush 開始より前に association outbound queue に未送信 envelope が残っている
- **WHEN** caller が flush session を開始しようとする
- **THEN** association または `Remote` の flush start path は flush request effect を返さない
- **AND** flush start failure または timeout outcome を観測可能にする
- **AND** flush completed を ordering guarantee として返してはならない

### Requirement: flush effects are transport-neutral

`AssociationEffect` は flush request の送信、flush completed、flush timed out、flush failed を transport-neutral な effect として表現する SHALL。effect は concrete TCP handle、tokio task、`JoinHandle`、channel sender を含んではならない（MUST NOT）。

#### Scenario: start flush returns send effects

- **WHEN** association が flush session を開始する
- **THEN** effect は remote authority、flush id、flush scope、対象 writer lane id、期待 ack 数を含む
- **AND** std adaptor はこの effect から `ControlPdu::FlushRequest` を作れる

#### Scenario: completed effect identifies original flush

- **WHEN** association が flush completed effect を返す
- **THEN** effect は flush id と flush scope を含む
- **AND** std adaptor は shutdown flush と DeathWatch notification 前 flush を区別できる

#### Scenario: timed out effect identifies remaining lanes

- **WHEN** association が flush timed-out effect を返す
- **THEN** effect は flush id、flush scope、ack 未到達の lane id を含む
- **AND** timeout は log または test-observable path に渡せる
