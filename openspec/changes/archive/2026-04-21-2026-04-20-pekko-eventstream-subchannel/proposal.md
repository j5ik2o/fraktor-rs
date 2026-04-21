## プロジェクト原則（全 change 共通）

本 change は以下 4 原則に従って設計される:

1. **Pekko 互換仕様の実現 + Rust らしい設計**: Pekko の `SubchannelClassification` の「variant 単位で購読者を絞る」本質を保ちつつ、Rust の closed enum 特性を活かして動的 `isAssignableFrom` ではなく `ClassifierKey::for_event` による静的 dispatch に翻訳する
2. **手間が掛かっても本質的な設計を選ぶ**: `subscribe` を糖衣構文として残すかどうかは API ergonomics の判断であり後方互換とは区別する。key 指定のない購読が頻出する実態を踏まえ、`subscribe_with_key(All, ...)` を冗長な標準形にしない
3. **フォールバックや後方互換性を保つコードを書かない**: `EventStreamEvent` の新 variant 追加時に `ClassifierKey` も拡張する契約を保ち、未登録 variant の fallback 配送を入れない（exhaustive match でコンパイル時検出）
4. **no_std core + std adaptor 分離**: 本 change は `modules/actor-core/src/core/kernel/event/stream/` のみを触り no_std を維持する。`alloc::*` のみ使用、`std::*` 禁止

## Why

Pekko `EventBus.scala:136-` の `SubchannelClassification` + `SubclassifiedIndex` は `Class` 階層 (`isAssignableFrom`) ベースで購読者を filter する仕組みで、購読時にイベント種別を絞り込める。fraktor-rs の `EventStreamEvent` は closed enum (14 variants) で、動的 classifier は YAGNI だが、「variant ごとに購読者を振り分ける」needs は gap-analysis ES-H1 項目として残っており、Phase A2+ の未対応項目として挙げられている。

現在のブランチには既に ES-H1 用テスト 4 件 (`modules/actor-core/src/core/kernel/event/stream/base/tests.rs:197-360`) が書かれているが、production code に `ClassifierKey` / `subscribe_with_key` が存在せずコンパイル不可状態。本 change は `ClassifierKey` enum と `subscribe_with_key` API を追加して既存テストを passing にする。

## What Changes

- `modules/actor-core/src/core/kernel/event/stream/classifier_key.rs` を新設（1 公開 enum 1 ファイル規則に従う）
  - `pub enum ClassifierKey { Lifecycle, Log, DeadLetter, Extension, Mailbox, MailboxPressure, UnhandledMessage, AdapterFailure, Serialization, RemoteAuthority, RemotingBackpressure, RemotingLifecycle, SchedulerTick, TickDriver, All }`
  - `impl ClassifierKey { pub fn for_event(event: &EventStreamEvent) -> Self }` で variant 対応関係を定義
- `EventStream::subscribe_with_key(&mut self, key, subscriber) -> (u64, Vec<EventStreamEvent>)` を追加
- `EventStream::subscribe(subscriber)` は `subscribe_with_key(ClassifierKey::All, subscriber)` の **糖衣構文（syntactic sugar）** として位置付ける
  - これは「後方互換性の保持」ではなく、「key 指定不要な通常購読」に対する API ergonomics の選択である（`All` 明示指定は Rust 的にはノイズとなるため、`subscribe` の形で簡潔な書き味を提供する）
  - subscribe を廃止して全 caller を `subscribe_with_key(All, ...)` に書き換える選択肢も検討したが、`ClassifierKey::All` が「kind 指定を省略する」という semantics と重複し冗長になるため、糖衣構文として残すのが本質的な設計と判断した
  - 既存 caller 20+ 箇所がそのまま動作するのは副次的な結果であって、後方互換のために維持するものではない
- `EventStream::publish_prepare(&mut self, event)` を subchannel aware に変更
  - 返る `Vec<EventStreamSubscriberEntry>` は `ClassifierKey::for_event(&event) == key` または `key == ClassifierKey::All` の subscriber のみを含む
- `EventStreamShared::subscribe_with_key` / `EventStreamShared::publish` を透過的に更新
- 内部データ構造 `EventStreamSubscriberEntries` に `ClassifierKey` タグを持たせる（または per-key `Vec` のマップ）

## Capabilities

### New Capabilities
- `pekko-eventstream-subchannel`: `EventStreamEvent` の variant 単位で購読者を絞り込む classifier 機構（kernel 層 `modules/actor-core/src/core/kernel/event/stream/` が対象）

注: 既存 capability `actor-typed-eventstream-package` は typed 層 (`modules/actor-core/src/core/typed/eventstream/`) の package 境界を定義しており、本 change が触る kernel 層 event stream とは対象が異なるため MODIFIED 不要

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/event/stream/classifier_key.rs` (新規)
  - `modules/actor-core/src/core/kernel/event/stream.rs` (mod 追加 + re-export)
  - `modules/actor-core/src/core/kernel/event/stream/base.rs` (`subscribe_with_key` / `publish_prepare` 改修)
  - `modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs` (透過ラップ)
  - `modules/actor-core/src/core/kernel/event/stream/event_stream_subscriber_entries.rs` (内部データ構造)
  - テスト: `modules/actor-core/src/core/kernel/event/stream/base/tests.rs` (既存 4 件が既に書かれている、passing にする)
- 影響内容:
  - 既存 `EventStream::subscribe(subscriber)` の挙動は非変更（`ClassifierKey::All` 相当）
  - `publish` での配送は subchannel aware になるが、既存 `All` 購読者は全 event を受け取るため観測可能な挙動変化なし
- 非目標:
  - 動的 `isAssignableFrom` 互換 classifier（closed enum のため YAGNI）
  - `EventStream::unsubscribe_all_from(key)` API（gap-analysis 未列挙）
  - Pekko `LookupClassification` / `ActorClassification` の移植

## 依存関係

- **`2026-04-20-pekko-restart-completion` を先に merge する必要がある**（同ブランチは kernel ビルドエラー状態のため、本 change の既存テスト `event::stream::base::tests::es_h1_*` も workspace ビルド不可）
- `2026-04-20-pekko-panic-guard` とは独立（モジュール境界が分かれており並列実装可）
