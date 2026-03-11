# actor モジュール ギャップ分析

参照実装: `references/pekko/actor-typed/`、`references/pekko/actor/`
対象実装: `modules/actor/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko actor-typed 公開型数（scaladsl、delivery 除く） | 約 55 |
| Pekko actor-typed 公開型数（delivery 含む） | 約 70 |
| fraktor-rs typed 公開型数 | 41 |
| fraktor-rs core 公開型数（全体） | 285 |
| Pekko classic actor 公開型数 | 約 182 |
| typed カバレッジ（delivery 除く） | 41/55 (≈75%) |
| typed ギャップ数 | 14 項目 |

---

## カテゴリ別ギャップ

### 1. コアAPI（ほぼ実装済み ✅）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Behavior` sentinel values (same/stopped/ignore/unhandled/empty) | `Behaviors.scala` | `behavior.rs:Behaviors::same/stopped/..` | - | ✅ 実装済み |
| `Behaviors.setup` | `Behaviors.scala:L40` | `behaviors.rs:Behaviors::setup` | - | ✅ |
| `Behaviors.withStash` | `Behaviors.scala:L46` | `behaviors.rs:Behaviors::with_stash` | - | ✅ |
| `Behaviors.receive/receiveMessage` | `Behaviors.scala:L115,134` | `behaviors.rs:Behaviors::receive_message` | - | ✅ |
| `Behaviors.receiveSignal` | `Behaviors.scala:L177` | `behaviors.rs:Behaviors::receive_signal` | - | ✅ |
| `Behaviors.supervise` | `Behaviors.scala:L250` | `supervise.rs:Supervise<M>` | - | ✅ |
| `Behaviors.withTimers` | `Behaviors.scala:L270` | `behaviors.rs:Behaviors::with_timers` | - | ✅ |
| `Behaviors.intercept` | `Behaviors.scala:L191` | `behaviors.rs:Behaviors::intercept` | - | ✅ |
| `Behaviors.monitor` | `Behaviors.scala:L207` | `behaviors.rs:Behaviors::monitor` | - | ✅ |
| `ActorContext.self` | `ActorContext.scala:L70` | `actor_context.rs:self_ref` | - | ✅ |
| `ActorContext.spawn/spawnAnonymous` | `ActorContext.scala:L134,142` | `actor_context.rs:spawn_child` | - | ✅ |
| `ActorContext.watch/watchWith/unwatch` | `ActorContext.scala:L178,193,202` | `actor_context.rs:watch/watch_with/unwatch` | - | ✅ |
| `ActorContext.setReceiveTimeout/cancel` | `ActorContext.scala:L213,221` | `actor_context.rs:set_receive_timeout` | - | ✅ |
| `ActorContext.messageAdapter` | `ActorContext.scala:L294` | `actor_context.rs:message_adapter` | - | ✅ |
| `ActorContext.ask/askWithStatus` | `ActorContext.scala:L319,328` | `actor_context.rs`（ActorRef 経由） | - | ✅ |
| `ActorContext.pipeToSelf` | `ActorContext.scala:L338` | `actor_context.rs:pipe_to_self` | - | ✅ |
| `SupervisorStrategy`（restart/backoff/stop） | `SupervisorStrategy.scala` | `supervision/base.rs`, `backoff_supervisor_strategy.rs` | - | ✅ |
| `TimerScheduler`（全メソッド） | `TimerScheduler.scala` | `timer_scheduler.rs` | - | ✅ |
| `StashBuffer`（基本操作 stash/unstashAll） | `StashBuffer.scala` | `stash_buffer.rs` | - | ✅ |
| `BehaviorInterceptor` | `BehaviorInterceptor.scala` | `behavior_interceptor.rs` | - | ✅ |
| `ServiceKey/Receptionist` | `Receptionist.scala` | `service_key.rs`, `receptionist.rs` | - | ✅ |
| `SpawnProtocol` | `SpawnProtocol.scala` | `spawn_protocol.rs` | - | ✅ |
| Signal 型（Terminated/ChildFailed/PostStop/PreRestart/MessageAdaptionFailure） | `MessageAndSignals.scala` | `behavior_signal.rs:BehaviorSignal` | - | ✅ |
| `StatusReply` | Pekko `StatusReply` | `status_reply.rs` | - | ✅ |
| `GroupRouter/PoolRouter` | `Routers.scala` | `group_router_builder.rs`, `pool_router_builder.rs` | - | ✅ |
| Extension API | `Extensions.scala` | `extension.rs`, `extension_id.rs` | - | ✅ |
| EventStream（typed） | `EventStream.scala` | `event/stream/event_stream_shared.rs` | - | ✅ 別名実装 |

---

### 2. 軽微なギャップ（StashBuffer 便利メソッド）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `StashBuffer.capacity()` | `StashBuffer.scala:L73` | 未対応 | trivial | `max_messages` フィールドから導出可能 |
| `StashBuffer.nonEmpty` | `StashBuffer.scala:L59` | 未対応 | trivial | `!is_empty()` と同等 |
| `StashBuffer.contains(message)` | `StashBuffer.scala:L111` | 未対応 | easy | メッセージ同一性チェック |
| `StashBuffer.exists(predicate)` | `StashBuffer.scala:L119` | 未対応 | easy | 述語によるサーチ |
| `StashBuffer.foreach(f)` | `StashBuffer.scala:L103` | 未対応 | easy | イテレーション |
| `StashBuffer.head` | `StashBuffer.scala:L95` | 未対応 | easy | 先頭要素参照 |
| `StashBuffer.clear()` | `StashBuffer.scala:L124` | 未対応 | easy | 全メッセージ廃棄 |
| `StashBuffer.unstash(n, wrap)` | `StashBuffer.scala:L165` | `stash_buffer.rs:unstash`（一部） | easy | N 件のみ処理する部分アンスタッシュ。`wrap` 変換関数付き |

---

### 3. ロギング関連

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Behaviors.logMessages(behavior)` | `Behaviors.scala:L215` | 未対応 | easy | メッセージ受信をデバッグログ出力するラッパー Behavior |
| `Behaviors.logMessages(opts, behavior)` | `Behaviors.scala:L223` | 未対応 | easy | `LogOptions` 付きバリアント |
| `Behaviors.withMdc(mdc, behavior)` | `Behaviors.scala:L285,299,321` | 未対応 | medium | MDC（Mapped Diagnostic Context）のロギング文脈設定。no_std 制約上実装要検討 |
| `LogOptions` 型 | `LogOptions.scala` | 未対応 | easy | ログ有効化フラグ・レベル・ロガーを束ねる設定型 |

---

### 4. ActorContext 追加メソッド

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ActorContext.delegate(delegator, msg)` | `ActorContext.scala:L152` | 未対応 | medium | 現在の Behavior を別 Behavior に委譲して処理させる。`Behaviors.same` を返す |
| `ActorContext.setLoggerName` | `ActorContext.scala:L99` | 未対応 | easy | ロガー名の動的変更。`log` メソッド前提なので低優先 |

---

### 5. 型抽象化

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RecipientRef[-T]` | `ActorRef.scala` | 未対応 | easy | `ActorRef` と typed `ActorRef` の共通スーパートレイト。`ask` パターンの対象を抽象化できる |
| `BehaviorSignalInterceptor[Inner]` | `BehaviorInterceptor.scala` | 未対応 | easy | シグナルのみ傍受する簡略版 `BehaviorInterceptor` |
| `ExtensionSetup[T]` | `Extensions.scala` | 未対応 | easy | ActorSystem ブートアップ時に Extension を設定する抽象基底型 |
| `ActorRefResolver` | `ActorRefResolver.scala` | 未対応 | medium | `ActorRef` を文字列にシリアライズ/デシリアライズする Extension |

---

### 6. Pub/Sub

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Topic`（pub/sub actor） | `pubsub/Topic.scala` | 未対応 | medium | トピックベースの Pub/Sub。`Receptionist` + 集約で代替可能だが、`Topic` は自律 Behavior として動作 |
| `Topic.Publish` コマンド | `pubsub/Topic.scala:L50` | 未対応 | medium | 上記に付随 |
| `Topic.Subscribe/Unsubscribe` | `pubsub/Topic.scala:L63,75` | 未対応 | medium | 上記に付随 |
| `Topic.GetTopicStats` | `pubsub/Topic.scala:L111` | 未対応 | easy | 統計取得（優先度低） |

---

### 7. 信頼性のあるメッセージ配信（Delivery）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ProducerController` | `delivery/ProducerController.scala` | 未対応 | hard | Point-to-point の信頼性メッセージ配信（プロデューサー側） |
| `ConsumerController` | `delivery/ConsumerController.scala` | 未対応 | hard | 同上（コンシューマー側）。シーケンス番号管理付き |
| `WorkPullingProducerController` | `delivery/WorkPullingProducerController.scala` | 未対応 | hard | ワーカープル型の負荷分散付き配信 |
| `DurableProducerQueue` | `delivery/DurableProducerQueue.scala` | 未対応 | hard | 耐久性のあるキューバックエンドの抽象化 |

---

### 8. シャットダウン

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `CoordinatedShutdown` | `actor/CoordinatedShutdown.scala` | 未対応 | hard | 多フェーズ順序付きシステムシャットダウン。クラスター統合と絡む |

---

### 9. 対象外（n/a）

| Pekko API | 理由 |
|-----------|------|
| `receivePartial / receiveMessagePartial` | Scala の `PartialFunction` は Rust に対応概念なし |
| `GroupRouter.preferLocalRoutees` | クラスター機能。単独 actor モジュールの範囲外 |
| `ActorContext.executionContext` | JVM `ExecutionContext` 固有 |
| `AbstractBehavior`（OOP 継承スタイル） | fraktor-rs の `TypedActor` trait が同等の役割を担う |
| `ActorSystem.Settings`（Typesafe Config） | JVM の HOCON 設定システム依存 |
| `ActorRefResolverSetup` | JVM setup 機構（ExtensionSetup 派生） |
| `Behaviors.receiveMessageWithSame` | `receive_message` で `Behavior::same()` を返せば同等 |
| `Behaviors.withMdc` | JVM MDC ログ固有。`tracing` クレートで代替 |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `StashBuffer.capacity()` — `max_messages` フィールド公開のみ
- `StashBuffer.nonEmpty` — `!is_empty()` の別名

### Phase 2: easy（単純な新規実装）

- `StashBuffer.contains / exists / foreach / head / clear` — コレクション操作の追加
- `StashBuffer.unstash(n, wrap)` — 部分アンスタッシュ
- `LogOptions` 型の追加 — ログ設定のバリューオブジェクト
- `Behaviors.logMessages` — デバッグ用メッセージロギング Behavior
- `RecipientRef` トレイト — `ask` 対象の抽象化
- `BehaviorSignalInterceptor` — シグナルのみ傍受する簡略 Interceptor
- `ExtensionSetup` — ブートアップ時の Extension 設定

### Phase 3: medium（中程度の実装工数）

- `Topic`（pub/sub）— `Receptionist` + EventStream ベースで実装可能だが自律 Behavior 設計が必要
- `ActorContext.delegate` — Behavior 委譲メカニズム
- `ActorRefResolver` — シリアライズ/パス解決 Extension
- `Behaviors.withMdc` — no_std 制約上の実現可能性を要検討

### Phase 4: hard（アーキテクチャ変更を伴う）

- Delivery patterns（`ProducerController` / `ConsumerController` / `WorkPullingProducerController` / `DurableProducerQueue`）— シーケンス番号管理・耐久キュー抽象化・バックプレッシャー統合が必要
- `CoordinatedShutdown` — クラスターモジュールと連携する多フェーズシャットダウン基盤

### 対象外（n/a）

- `receivePartial`、`receiveMessagePartial`、`AbstractBehavior`、`preferLocalRoutees`、`executionContext`、`ActorSystem.Settings`、`Behaviors.receiveMessageWithSame`、`Behaviors.withMdc`

---

## 所見

fraktor-rs の actor モジュールは Pekko の typed API コアの約 75% をカバーしており、主要ユースケース（Behavior 設計・監視・タイマー・スタッシュ・ルーター・レセプショニスト）は実装済みです。

主な未実装領域は次の3つです：

1. **StashBuffer の便利メソッド群**（easy、YAGNI 判断で後回し可）
2. **Pub/Sub（`Topic`）** — Receptionist と組み合わせれば代替可能だが、よく使われるパターン
3. **Delivery パターン** — 信頼性保証が必要なシステム向け。現フェーズでは不要と判断して差し支えない（hard）
