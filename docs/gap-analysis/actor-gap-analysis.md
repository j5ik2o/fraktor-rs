# actor モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（意味のある型単位） | 約 95（classic: ~45, typed: ~50） |
| fraktor-rs 公開型数 | 約 85（core/kernel: ~40, core/typed: ~30, std: ~15） |
| カバレッジ（型単位） | 約 75/95 (79%) |
| ギャップ数 | 20（core: 8, std: 4, n/a: 8） |

※ Java API 重複（AbstractBehavior, ReceiveBuilder 等）、private[pekko] 内部型、例外クラス群は Pekko 側計数から除外。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | ~45 | ~40 | 89% |
| core / typed ラッパー | ~50 | ~30 | 60% |
| std / アダプタ | （実装固有） | ~15 | — |

## カテゴリ別ギャップ

### コア型（ActorRef, ActorSystem, Props, ActorContext） ✅ 実装済み 12/12 (100%)

全主要型が実装済み。ギャップなし。

### Address / ActorPath ✅ 実装済み 5/7 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RootActorPath` | `ActorPath.scala:278` | 未対応 | core/kernel | easy | ActorPath にルート/子の区別がない |
| `ChildActorPath` | `ActorPath.scala:327` | 未対応 | core/kernel | easy | 同上。現状 ActorPath は単一型 |

### メッセージ型・シグナル ✅ 実装済み 9/13 (69%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PoisonPill` | `Actor.scala:46` | 未対応 | core/kernel | easy | graceful_stop で代替可能だが、Pekko 互換性のためあると便利 |
| `Kill` | `Actor.scala:60` | 未対応 | core/kernel | easy | 即時停止シグナル |
| `ChildFailed` | `MessageAndSignals.scala:104` | 未対応 | core/typed | medium | SupervisorStrategy との連携が必要 |
| `MessageAdaptionFailure` | `MessageAndSignals.scala:125` | 未対応 | core/typed | trivial | MessageAdapter のエラーハンドリング |

### Behaviors ファクトリ ✅ 実装済み 12/14 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Behaviors.withTimers` | `Behaviors.scala:167` | 未対応 | core/typed | medium | TimerScheduler は存在するが Behaviors 経由のファクトリがない |
| `Behaviors.supervise` | `Behaviors.scala:243` | 別名で実装済み | core/typed | — | `Supervise<M>` ビルダーとして存在 |

### Supervision ✅ 実装済み 4/4 (100%)

全主要型（SupervisorStrategy, BackoffSupervisorStrategy, SupervisorDirective, Supervise）が実装済み。

### BehaviorInterceptor ✅ 実装済み 3/3 (100%)

BehaviorInterceptor, BehaviorSignalInterceptor, PreStartTarget 相当が実装済み。

### Receptionist / ServiceKey ✅ 実装済み 6/7 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Deregister` | `Receptionist.scala:223` | 未対応 | core/typed | easy | 登録解除コマンド。Register はあるが Deregister がない |

### Router ✅ 実装済み 3/3 (100%)

Routers, PoolRouterBuilder, GroupRouterBuilder が実装済み。

### Timer / Stash ✅ 実装済み 4/4 (100%)

TimerScheduler, TimerKey, StashBuffer が実装済み。

### Ask パターン ✅ 実装済み 4/4 (100%)

ask_with_timeout, ask on context, ask_with_status, StatusReply が実装済み。

### EventStream ✅ 実装済み 3/4 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Unsubscribe[E]` | `EventStream.scala:74` | 未対応 | std | easy | Subscribe はあるが Unsubscribe がない |

### Topic / PubSub ✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GetTopicStats` | `Topic.scala:111` | 未対応 | core/typed | trivial | TopicStats は存在するが取得コマンドがない |

### Extension ✅ 実装済み 4/4 (100%)

Extension, ExtensionId, ExtensionInstaller が実装済み。

### FSM ✅ 実装済み 1/1 (100%)

FSMBuilder が typed 層に存在。Pekko の classic FSM は複雑な Scala DSL だが、fraktor-rs は typed 層で簡潔に実装。

### CoordinatedShutdown ❌ 未対応 0/1

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CoordinatedShutdown` | `CoordinatedShutdown.scala:41` | 未対応 | std | hard | フェーズ付きシャットダウンオーケストレーション。ActorSystem 終了時のリソース解放順序制御 |

### SpawnProtocol ❌ 未対応 0/1

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SpawnProtocol` | `SpawnProtocol.scala:36` | 未対応 | core/typed | medium | 外部からメッセージ経由でアクターを生成するプロトコル |

### Delivery（信頼性メッセージング） ❌ 未対応 0/2

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableProducerQueue` | `DurableProducerQueue.scala:33` | 未対応 | core/typed + std | hard | 永続化キュー。persistence モジュールとの統合が必要 |
| `ConsumerController` | `ConsumerController.scala:60` | 未対応 | core/typed | hard | フロー制御付きメッセージ配信 |

### ActorRefResolver ❌ 未対応 0/1

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorRefResolver` | `ActorRefResolver.scala:20` | 未対応 | core/typed | medium | シリアライゼーション用の ActorRef ⇔ 文字列変換。remote モジュールで必要になる |

### 対象外（n/a）

| Pekko API | 理由 |
|-----------|------|
| Java API 重複（`javadsl.*`） | Rust に Java API は不要 |
| `AbstractBehavior[T]` | OOP パターン。Rust では Behavior<M> クロージャで代替 |
| `ReceiveBuilder` / `BehaviorBuilder` | Java Builder パターン。Rust では不要 |
| `Adapter` (classic/typed interop) | fraktor-rs は typed 優先設計。classic 互換層は不要 |
| `Deploy` / `Deployer` | JVM 固有のリモートデプロイ設定 |
| `AbstractFSM` / `AbstractLoggingFSM` | Java API。FSMBuilder で代替 |
| `ActorLogging` / `DiagnosticActorLogging` | Rust は tracing クレートで対応 |
| Classic `Stash` / `UnboundedStash` | typed 層の StashBuffer で統一 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `MessageAdaptionFailure` (core/typed) — MessageAdapter のエラー通知シグナル
- `GetTopicStats` (core/typed) — TopicStats 取得コマンド追加

### Phase 2: easy（単純な新規実装）
- `PoisonPill` / `Kill` (core/kernel) — 標準停止メッセージ
- `Deregister` (core/typed) — Receptionist の登録解除
- `Unsubscribe` (std) — EventStream の購読解除
- `Behaviors.withTimers` (core/typed) — TimerScheduler 統合ファクトリ
- `RootActorPath` / `ChildActorPath` (core/kernel) — ActorPath の型区別

### Phase 3: medium（中程度の実装工数）
- `ChildFailed` (core/typed) — 子アクター失敗シグナル（Supervision 連携）
- `SpawnProtocol` (core/typed) — メッセージ経由のアクター生成
- `ActorRefResolver` (core/typed) — ActorRef シリアライゼーション（remote 前提）

### Phase 4: hard（アーキテクチャ変更を伴う）
- `CoordinatedShutdown` (std) — フェーズ付きシャットダウン。ActorSystem のライフサイクルに深く関わる
- `DurableProducerQueue` / `ConsumerController` (core/typed + std) — 永続化 + フロー制御。persistence モジュール統合が前提

### 対象外（n/a）
- Java API 重複、Classic 互換層、JVM 固有機能（上記 n/a テーブル参照）

## まとめ

- **全体カバレッジ 79%**: 主要機能（ActorRef, Behavior, Supervision, Receptionist, Router, Ask, Timer, Stash, Topic, Extension）はほぼカバー済み
- **即座に価値を提供できる未実装**: `Behaviors.withTimers`（Phase 2）は利便性が高い。`PoisonPill`/`Kill` は Pekko 利用者にとって馴染みのある API
- **実用上の主要ギャップ**: `CoordinatedShutdown`（Phase 4）は本番運用で重要。`ActorRefResolver` は remote モジュール統合時に必須
- **YAGNI 観点での省略推奨**: Delivery（DurableProducerQueue/ConsumerController）は persistence モジュールの成熟後に検討すべき。Java API 重複・Classic 互換層は不要
