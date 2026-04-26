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

`Association` 内の `SendQueue` は system priority (system message) と user priority (user message) の2つのキューを持ち、system 優先で取り出す SHALL。`Association::next_outbound` と `Association::apply_backpressure` は内部の `SendQueue` に委譲することで、この priority ロジックを公開する。

#### Scenario: system queue の優先

- **WHEN** system priority と user priority の両方のメッセージが queue にあり、`Association::next_outbound()` を呼ぶ
- **THEN** system priority のメッセージが先に返される

#### Scenario: user queue の backpressure pause

- **WHEN** `Association` に `apply_backpressure(BackpressureSignal::Apply)` を適用してから `next_outbound()` を呼ぶ
- **THEN** user priority のメッセージは取り出されず、system priority のみ取り出される

#### Scenario: backpressure release

- **WHEN** `apply_backpressure(BackpressureSignal::Release)` を適用してから `next_outbound()` を呼ぶ
- **THEN** user priority のメッセージも取り出される

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

