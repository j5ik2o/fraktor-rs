## GitHub Issue #759: feat(actor): Pekko actor-typed ギャップ解消

## 概要

`docs/gap-analysis/actor-gap-analysis.md` に基づく Pekko actor-typed との未対応機能をまとめて追跡するissue。

Phase 1〜3までを対応します。Phase 4は複雑過ぎるので別issueで対応します。

ただし注意があります。構造的には

- `modules/actor/src/core`はuntyped実装が中心、`modules/actor/src/core/typed`がtyped実装です。
- `modules/actor/src/std`は、`modules/actor/src/core`にあるポートに対応するstd用アダプタ実装だけを配置してください。

となっています。

typed実装を作るならまずuntyped実装がなければなりません。untypedはclassicではなく基盤実装です。ほとんどのロジックはuntypedに集中するように設計しなければなりません。アクターライブラリ内部で型を意識すると複雑に成りすぎるからです。一方で、ユーザに近いインターフェイス部分では、型付きラッパーとしてtyped実装を作って提供することになります。

これらの設計指針については既存実装の構造をよく見て実装してください。

---

## Phase 1: trivial

- [ ] `StashBuffer::capacity()` — `max_messages` フィールド公開のみ

---

## Phase 2: easy

### StashBuffer 便利メソッド

- [ ] `StashBuffer::contains(message)` — メッセージ同一性チェック（`StashBuffer.scala:L111`）
- [ ] `StashBuffer::exists(predicate)` — 述語によるサーチ（`StashBuffer.scala:L119`）
- [ ] `StashBuffer::foreach(f)` — イテレーション（`StashBuffer.scala:L103`）
- [ ] `StashBuffer::head` — 先頭要素参照（`StashBuffer.scala:L95`）
- [ ] `StashBuffer::clear()` — 全メッセージ廃棄（`StashBuffer.scala:L124`）
- [ ] `StashBuffer::unstash(n, wrap)` — N件のみ処理する部分アンスタッシュ（`StashBuffer.scala:L165`）

### ルーター戦略

- [ ] `GroupRouterBuilder::with_random_routing()` — ランダムルーティング（現在はラウンドロビン固定）
- [ ] `PoolRouterBuilder::with_round_robin()` — ラウンドロビン戦略
- [ ] `PoolRouterBuilder::with_broadcast_predicate(f)` — フィルタ付きブロードキャスト

### ロギング関連

- [ ] `LogOptions` 型 — ログ有効化フラグ・レベル・ロガー名を束ねる設定型（`LogOptions.scala`）
- [ ] `Behaviors::log_messages(behavior)` — メッセージ受信をデバッグログ出力するラッパー Behavior（`Behaviors.scala:L215`）
- [ ] `Behaviors::log_messages_with_opts(opts, behavior)` — `LogOptions` 付きバリアント（`Behaviors.scala:L223`）

### 型抽象化

- [ ] `RecipientRef<T>` トレイト — `ActorRef` と `TypedActorRef<M>` の共通スーパートレイト（`ask` 対象の抽象化）
- [ ] `BehaviorSignalInterceptor<Inner>` — シグナルのみ傍受する簡略版 `BehaviorInterceptor`
- [ ] `ExtensionSetup<T>` — ActorSystem 起動時に Extension を設定する抽象基底型

---

## Phase 3: medium

- [ ] `GroupRouterBuilder::with_consistent_hash_routing(f)` — コンシステントハッシュルーティング
- [ ] `TypedActorContext::delegate(delegatee, msg)` — 現在のメッセージを別 Behavior に委譲（`ActorContext.scala:L152`）
- [ ] `ActorRefResolver` Extension — `ActorRef` を文字列にシリアライズ/デシリアライズ
- [ ] `Topic`（pub/sub actor） — トピックベースの Pub/Sub Behavior（`pubsub/Topic.scala`）

---

## 参照

- ギャップ分析: `docs/gap-analysis/actor-gap-analysis.md`
- Pekko 参照実装: `references/pekko/actor-typed/`

### Labels
enhancement, actor, compatibility