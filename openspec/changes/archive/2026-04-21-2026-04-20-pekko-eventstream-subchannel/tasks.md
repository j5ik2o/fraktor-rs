## 0. 前提確認

- [x] 0.1 既存テスト `modules/actor-core/src/core/kernel/event/stream/base/tests.rs` の `es_h1_t1..t4` が存在することを確認する
- [x] 0.2 `EventStreamEvent` の variant 14 種類を列挙確認する (`event_stream_event.rs`)

## 1. `ClassifierKey` enum の新設

- [x] 1.1 `modules/actor-core/src/core/kernel/event/stream/classifier_key.rs` を新規作成
  - `pub enum ClassifierKey` に `Lifecycle`, `Log`, `DeadLetter`, `Extension`, `Mailbox`, `MailboxPressure`, `UnhandledMessage`, `AdapterFailure`, `Serialization`, `RemoteAuthority`, `RemotingBackpressure`, `RemotingLifecycle`, `SchedulerTick`, `TickDriver`, `All` を定義
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` を付与
- [x] 1.2 `impl ClassifierKey { pub fn for_event(event: &EventStreamEvent) -> Self }` を実装
  - 各 variant (`Lifecycle(_) => Self::Lifecycle`, ...) の対応関係を網羅
  - `All` は戻り値としては返さない（`for_event` の戻りは具象 variant のみ）
- [x] 1.3 `modules/actor-core/src/core/kernel/event/stream.rs` に `mod classifier_key;` + `pub use classifier_key::ClassifierKey;` を追加
- [x] 1.4 `classifier_key/tests.rs` を新設し、全 14 variant の `for_event` 戻り値を網羅するテストを追加
- [x] 1.5 `rtk cargo check -p fraktor-actor-core-rs` でクリーンビルドを確認
- [x] 1.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 2. `EventStreamSubscriberEntries` の内部データ構造拡張

- [x] 2.1 `event_stream_subscriber_entries.rs` を改修: **subscriber entry に `ClassifierKey` フィールドを持たせる tagged `Vec<Entry>` 構成**を採用する
  - 理由: 現状 subscriber 数は小規模（数件〜数十件程度、cluster-core / cluster-adaptor-std 等を含めても数十件上限）。O(n) filter のオーバーヘッドは無視できる一方、`BTreeMap<ClassifierKey, Vec<Entry>>` は add/remove のロック区間複雑化と購読者数が少ない場合の定数倍コストが目立つため採用しない
  - entry 型に `key: ClassifierKey` フィールドを追加、または `(id, subscriber, key)` タプルとして保持
- [x] 2.2 `fn snapshot_for(&self, key: ClassifierKey) -> Vec<EventStreamSubscriberEntry>` を追加
  - `key == ClassifierKey::All` なら全 subscriber を返す
  - 具象 key なら `entry.key == key` または `entry.key == ClassifierKey::All` の subscriber を返す
- [x] 2.3 既存 `snapshot()` は `snapshot_for(ClassifierKey::All)` と等価になるように整理
- [x] 2.4 `add` / `remove` API は既存シグネチャ互換のまま、内部で key を保持
  - 既存 `add(subscriber)` は `ClassifierKey::All` で追加するラッパー、新 `add_with_key(key, subscriber)` を追加
- [x] 2.5 将来 subscriber 数が数百件以上になった場合は `BTreeMap` への内部切替を別 change で検討する旨を rustdoc に記載
- [x] 2.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 3. `EventStream::subscribe_with_key` / `publish_prepare` の改修

- [x] 3.1 `EventStream::subscribe_with_key(&mut self, key: ClassifierKey, subscriber: EventStreamSubscriberShared) -> (u64, Vec<EventStreamEvent>)` を追加
  - 既存 `subscribe` と同じく replay snapshot を返す
  - **replay snapshot も `ClassifierKey::for_event(&event) == key` または `key == ClassifierKey::All` を満たす event のみに絞る**（購読者側任せにしない）
  - 具体的には `EventStreamEvents::snapshot_for_key(key) -> Vec<EventStreamEvent>` を新設し、`snapshot()` はこれを `ClassifierKey::All` で呼ぶラッパーに整理
  - `EventStreamShared::subscribe_with_key` は kernel 側呼び出し後、返却された snapshot をそのまま subscriber へ replay する（subscriber 側で再フィルタしない）
- [x] 3.2 `EventStream::subscribe(subscriber)` を `subscribe_with_key(ClassifierKey::All, subscriber)` の糖衣構文として整理（内部は `subscribe_with_key` に委譲し、挙動差を作らない）
- [x] 3.3 `EventStream::publish_prepare(&mut self, event)` を改修
  - event 格納は既存ロジック維持
  - `ClassifierKey::for_event(&event)` を計算
  - `subscribers.snapshot_for(key)` で該当購読者のみを返す
- [x] 3.4 `subscribe_no_replay_with_key` は新設しない（`subscribe_no_replay` は `ClassifierKey::All` のまま据え置き、不要なら本 change でも未対応で良い）
- [x] 3.5 replay フィルタが効いていることを裏取りするテストを追加する
  - 例: `replay_filters_buffered_events_by_key` として、Log 購読者が `subscribe_with_key(ClassifierKey::Log, ...)` したとき、事前に buffered された Lifecycle event は replay されないことを検証
- 注: Pekko `SubchannelClassification` 互換の対象は classifier による配送絞り込みであり、購読時 replay と並行 publish の厳密順序保証までは本 change の対象外とする。既存 `EventStream` の replay 契約は維持する。
- [x] 3.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 4. `EventStreamShared` の透過対応

- [x] 4.1 `event_stream_shared.rs` に `pub fn subscribe_with_key(&self, key: ClassifierKey, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription` を追加
- [x] 4.2 既存 `subscribe` は内部で `subscribe_with_key(ClassifierKey::All, ...)` に委譲
- [x] 4.3 `publish(&self, event: &EventStreamEvent)` の内部配送ループで `snapshot_for(ClassifierKey::for_event(event))` を使う
- [x] 4.4 `./scripts/ci-check.sh ai dylint` が exit 0

## 5. 検証

- [x] 5.1 `rtk cargo test -p fraktor-actor-core-rs event::stream::base::tests::es_h1` で既存 4 件 + 追加 1 件 (replay filter test) が passing
- [x] 5.2 既存 `EventStream` 利用側（cluster-core, cluster-adaptor-std, 20+ 箇所）が回帰していないこと（`rtk cargo test --workspace -p fraktor-actor-core-rs -p fraktor-cluster-core-rs -p fraktor-cluster-adaptor-std-rs`）
- [x] 5.3 section 1〜4 の各末尾で `./scripts/ci-check.sh ai dylint` を実行済みであることを再確認（本項目で追加実行する必要はない。`ai all` に dylint が含まれるため section 6.5 で最終実行される）

## 6. 品質ゲート（マージ前 MUST 条件）

本 change が proposal の 4 原則を満たしていることをマージ前に以下の項目で機械的に裏取りする。1 つでも fail したら該当作業に戻す。

### 6.1 原則 2 (本質的な設計を選ぶ) のゲート

- [x] 6.1.1 `ClassifierKey` enum が `EventStreamEvent` の全 14 variant + `All` を網羅していること（Rust の exhaustive match でコンパイル時に検出されるため、match arm に `_ =>` ワイルドカードを使わない）
- [x] 6.1.2 `ClassifierKey::for_event(&event)` が具象 variant のみ返し、`All` は戻り値としない（仕様要件、match は exhaustive）
- [x] 6.1.3 `subscribe` と `subscribe_with_key(All, ...)` が **同一コードパス**を通ること（片方にだけロジックを書いて挙動乖離を作らない）
  - `rtk grep -n "fn subscribe\b\|fn subscribe_with_key" modules/actor-core/src/core/kernel/event/stream/` で `subscribe` が `subscribe_with_key` に委譲している実装であることを確認

### 6.2 原則 3 (後方互換性を保つコードを書かない) のゲート

- [x] 6.2.1 fallback 配送ロジックが存在しないこと
  - 未登録 `ClassifierKey` に対する fallback 配送コードがないこと（exhaustive match により compile error で検出）
  - `_ => <defaultへの配送>` 形式のワイルドカードが `EventStream` 関連コードに 0 件
- [x] 6.2.2 `subscribe` が後方互換のために残されている旨の記述がコード・rustdoc に**ない**こと
  - rustdoc は「`subscribe_with_key(ClassifierKey::All, ...)` の糖衣構文」「API ergonomics の選択」と記述し、「後方互換」「互換性維持」「既存 caller を壊さないため」の表現を使わない
- [ ] 6.2.3 未使用 variant / 未使用 field / 未使用 method が 0 件
  - `rtk cargo clippy -p fraktor-actor-core-rs --all-targets -- -D dead_code` が exit 0
- [x] 6.2.4 暫定 fallback / legacy alias が 0 件
  - `rtk grep -rn "legacy\|compat\|deprecated\|backwards" modules/actor-core/src/core/kernel/event/stream/` で本 change 由来の互換コードがないこと
  - 注: 既存の `unhandled_message.rs` にある `Classic-compatible alias` rustdoc は本 change 以前から存在しており、本 change では互換コードを追加していない

### 6.3 原則 4 (no_std core + std adaptor 分離) のゲート

- [x] 6.3.1 `rtk grep -rn "^use std::\|^use std$" modules/actor-core/src/core/kernel/event/stream/` が 0 件（`alloc::*` のみ使用）
- [x] 6.3.2 `cfg-std-forbid` dylint が違反を検出しないこと（下記 6.5.1 に含まれる）

### 6.4 Pekko 参照実装 parity のゲート

- [x] 6.4.1 本 change が proposal で参照している Pekko `EventBus.scala:136-` (`SubchannelClassification` / `SubclassifiedIndex`) の「variant 単位で購読者を絞る」本質が保たれていること
  - 実装が `ClassifierKey::for_event` による静的 dispatch で、Pekko の `isAssignableFrom` 動的判定を翻訳したものであること

### 6.5 CI / lint の final ゲート

- [x] 6.5.1 `./scripts/ci-check.sh ai all` が最終動作確認として exit 0（内部で dylint / cargo test / clippy / fmt を全件実行。8 custom lint 全 pass: mod-file / module-wiring / type-per-file / tests-location / use-placement / rustdoc / cfg-std-forbid / ambiguous-suffix。TAKT ルール上、このゲートは change のマージ直前にのみ実行）
  - 注: `rtk cargo clippy -p fraktor-actor-core-rs --all-targets -- -D dead_code` は `modules/actor-core/src/tests.rs` など change 外の既存 test-only clippy 違反で失敗するため、6.2.3 のみ未完了
