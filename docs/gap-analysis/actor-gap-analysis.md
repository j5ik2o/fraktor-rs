# actor モジュール ギャップ分析

> 分析日: 2026-02-27（前回: 2026-02-24）
> 対象: `modules/actor/src/` vs `references/pekko/actor-typed/src/` + `references/pekko/actor/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（actor-typed） | ~80（Behavior, ActorRef, Receptionist, Routers 等含む） |
| fraktor-rs 公開型数 | ~200+（17 ドメインに分散） |
| カバレッジ（型単位） | ~92%（直接対応する型ベース） |
| 未実装ギャップ数 | 6（前回12 → 6件削減） |

### 前回分析からの変更

以下の機能が新たに実装済みとなった：
- `Behaviors.withTimers` / `TimerScheduler` → 完全実装（`behaviors.rs:190-210`, `timer_scheduler.rs`）
- `BehaviorInterceptor` / `Behaviors.intercept` → 完全実装（`behavior_interceptor.rs`, `behaviors.rs:217-262`）
- `ActorContext.setReceiveTimeout` → 完全実装（`receive_timeout_config.rs`, `actor_context.rs:274-277`）
- `watchWith(target, msg)` → 完全実装（Typed: `actor_context.rs:136-140`, Untyped: `actor_context.rs:267-280`）

### 設計上の差異

- **実行モデル**: fraktor-rs は tick ベースの同期実行モデル。Pekko の `Future[T]` / `FiniteDuration` は tick / `ActorFuture` で代替
- **型パラメータ**: fraktor-rs は `Generic<TB: RuntimeToolbox>` パターンで no_std/std 両対応。Pekko は JVM 前提
- **Untyped + Typed**: fraktor-rs は Untyped Actor（`ActorLifecycle` trait）と Typed Actor（`Behavior<M, TB>`）の両方を提供。Pekko の classic actor と typed actor に相当
- **シリアライゼーション**: fraktor-rs は独自の `SerializationExtension` + `SerializationRegistry` を持ち、Pekko の `Serialization` 拡張と同等の機能を提供
- **リモーティング**: fraktor-rs は `RemoteWatchHook` / `ActorRefProvider` / `RemoteAuthorityResolver` で抽象化。Pekko の Artery/Classic remoting に相当する抽象化層

---

## カテゴリ別ギャップ

### 1. サービスディスカバリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Receptionist` | receptionist/Receptionist.scala | 未対応 | hard | アクター検索・購読・サービス登録。分散レジストリ基盤が必要 |
| `ServiceKey[T]` | receptionist/ServiceKey.scala | 未対応 | hard | 型安全なサービス登録キー。Receptionist に依存 |
| `Receptionist.Register` | receptionist/Receptionist.scala | 未対応 | hard | Receptionist プロトコルメッセージ |
| `Receptionist.Subscribe` | receptionist/Receptionist.scala | 未対応 | hard | サービス一覧の変更通知購読 |
| `Receptionist.Find` | receptionist/Receptionist.scala | 未対応 | hard | サービスの一回限りの検索 |

### 2. ルーティング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Routers.pool(size, behavior)` | scaladsl/Routers.scala | 未対応 | medium | 固定サイズのプールルーター |
| `Routers.group(serviceKey)` | scaladsl/Routers.scala | 未対応 | medium | ServiceKey ベースのグループルーター。Receptionist に依存 |
| `PoolRouter[T]` | PoolRouter.scala | 未対応 | medium | プールルーターの型 |
| `GroupRouter[T]` | GroupRouter.scala | 未対応 | medium | グループルーターの型 |

### 3. シグナル拡張

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ChildFailed` | Signal.scala | 未対応 | easy | 子アクター失敗シグナル。現在は SupervisorStrategy 経由で処理 |
| `PreRestart` | Signal.scala | 未対応 | easy | リスタート前シグナル。Lifecycle フックで代替可能 |
| `MessageAdaptionFailure` | Signal.scala | 未対応 | trivial | メッセージアダプタ変換失敗時のシグナル |

---

## 実装済み（Pekko に対してカバー済みの主要 API）

### コア型

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Behavior[T]` | `Behavior<M, TB>` | 完全。`BehaviorGeneric<M, TB>` + 型エイリアス |
| `ActorRef[T]` | `TypedActorRefGeneric<M, TB>` | 完全。Untyped `ActorRef` も別途存在 |
| `ActorSystem[T]` | `TypedActorSystemGeneric<M, TB>` | 完全。Untyped `ActorSystemGeneric<TB>` も存在 |
| `ActorContext[T]` | `TypedActorContextGeneric<M, TB>` | 完全。`spawn_child`, `spawn_child_watched`, `spawn_message_adapter` 等 |
| `Props` | `PropsGeneric<TB>` / `TypedPropsGeneric<M, TB>` | 完全 |

### Behaviors ファクトリ

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Behaviors.same` | `Behaviors::same` | 完全 |
| `Behaviors.stopped` | `Behaviors::stopped` | 完全 |
| `Behaviors.empty` | `Behaviors::empty` | 完全 |
| `Behaviors.unhandled` | `Behaviors::unhandled` | 完全 |
| `Behaviors.setup(f)` | `Behaviors::setup(f)` | 完全 |
| `Behaviors.receive(f)` | `Behaviors::receive_message(f)` | 完全（別名） |
| `Behaviors.receiveMessage(f)` | `Behaviors::receive_message(f)` | 完全 |
| `Behaviors.receiveSignal(f)` | `Behaviors::receive_signal(f)` | 完全 |
| `Behaviors.withStash(capacity)(f)` | `Behaviors::with_stash(capacity, f)` | 完全 |
| `Behaviors.supervise(behavior)` | `Behaviors::supervise(behavior)` | 完全。`Supervise::on_failure(strategy)` |
| `Behaviors.withTimers(f)` | `Behaviors::with_timers(f)` | **新規実装** |
| `Behaviors.intercept(interceptor)(behavior)` | `Behaviors::intercept(interceptor, behavior)` | **新規実装** |

### タイマー

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `TimerScheduler[T]` | `TimerSchedulerGeneric<M, TB>` | **新規実装**。`start_timer_with_fixed_delay`, `start_timer_at_fixed_rate`, `start_single_timer`, `cancel`, `cancel_all` |
| `TimerScheduler.startTimerWithFixedDelay` | `start_timer_with_fixed_delay()` | 完全 |
| `TimerScheduler.startTimerAtFixedRate` | `start_timer_at_fixed_rate()` | 完全 |
| `TimerScheduler.startSingleTimer` | `start_single_timer()` | 完全 |
| `TimerScheduler.isTimerActive` | `is_timer_active()` | 完全 |
| `TimerScheduler.cancel` | `cancel()` | 完全 |
| `TimerScheduler.cancelAll` | `cancel_all()` | 完全 |

### ビヘイビアインターセプタ

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `BehaviorInterceptor[Outer, Inner]` | `BehaviorInterceptorGeneric<M, TB>` | **新規実装**。`around_start`, `around_receive`, `around_signal` |

### ウォッチ

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `ActorContext.watch(target)` | `watch(target)` | 完全 |
| `ActorContext.watchWith(target, msg)` | `watch_with(target, msg)` | **新規実装**。Typed/Untyped 両方で実装 |

### タイムアウト

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `ActorContext.setReceiveTimeout(timeout, msg)` | `set_receive_timeout(config)` | **新規実装**。`ReceiveTimeoutConfig` で設定 |
| `ActorContext.cancelReceiveTimeout` | `cancel_receive_timeout()` | **新規実装** |

### シグナル

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Terminated` | `BehaviorSignal::Terminated` / `SystemMessage::Terminated` | 完全 |
| `PostStop` | ライフサイクルフック（`post_stop`） | 完全 |

### スーパービジョン

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `SupervisorStrategy` | `SupervisorStrategy` | 完全。`OneForOne`, `AllForOne` |
| `SupervisorStrategy.restart` | `SupervisorDirective::Restart` | 完全 |
| `SupervisorStrategy.stop` | `SupervisorDirective::Stop` | 完全 |
| `SupervisorStrategy.resume` | `SupervisorDirective::Resume` | 完全 |
| `Backoff` 戦略 | `max_retries` + `within_time_range` パラメータ | 完全 |

### StashBuffer

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `StashBuffer[T]` | `StashBufferGeneric<M, TB>` | 完全。bounded stash |
| `StashBuffer.stash(msg)` | `StashBuffer::stash(msg)` | 完全 |
| `StashBuffer.unstashAll(behavior)` | `StashBuffer::unstash_all(ctx)` | 完全 |
| `StashBuffer.size` / `isEmpty` / `isFull` | 同名メソッド | 完全 |

### Ask パターン

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `ActorRef.ask[Res](f)(timeout)` | `TypedActorRef::ask::<R, F>(build)` | 完全 |
| Ask レスポンス型 | `TypedAskResponse` / `TypedAskFuture` | 完全 |
| Ask エラー型 | `TypedAskError` | 完全 |

### 拡張システム

| Pekko API | fraktor対応 | 備考 |
|-----------|-------------|------|
| `Extension` | `Extension<TB>` trait | 完全 |
| `ExtensionId[T]` | `ExtensionId<TB>` trait | 完全 |
| `ExtensionSetup` | `ExtensionInstaller<TB>` trait | 完全（別名） |
| `ActorSystem.registerExtension` | `ExtendedActorSystem::register_extension` | 完全 |

### カテゴリ別カバー状況

| カテゴリ | カバー状況 |
|----------|-----------|
| Behavior ファクトリ（same, stopped, setup, receive, withTimers, intercept 等） | 完全 |
| 型付きアクターリファレンス（tell, ask, spawn） | 完全 |
| スーパービジョン（OneForOne, AllForOne, Directive） | 完全 |
| StashBuffer | 完全 |
| Ask パターン | 完全 |
| 拡張システム（Extension, ExtensionId） | 完全 |
| シリアライゼーション | 完全（独自の SerializationExtension + Registry） |
| EventStream / DeadLetter | 完全（EventStreamEvent + DeadLetterEvent） |
| ガーディアン（Root / System / User） | 完全 |
| メールボックス（Mailbox, Dispatcher） | 完全（インストルメンテーション付き） |
| リモートフック（RemoteWatchHook, ActorRefProvider） | 完全 |
| アクターライフサイクル（pre_start, post_stop, pre_restart, post_restart） | 完全 |
| タイマー（TimerScheduler, withTimers） | **完全（新規）** |
| ビヘイビアインターセプタ（intercept） | **完全（新規）** |
| アイドルタイムアウト（setReceiveTimeout） | **完全（新規）** |
| ウォッチ拡張（watchWith） | **完全（新規）** |

### fraktor-rs 独自の追加機能

| 機能 | 備考 |
|------|------|
| `Generic<TB: RuntimeToolbox>` パターン | no_std/std 両対応の汎用抽象化 |
| `TickDriver` / `TickDriverConfig` | tick ベースの同期実行エンジン |
| `MailboxInstrumentation` | メールボックスの監視・計測 |
| `SerializationRegistry` + `SerializerBinding` | 型安全なシリアライザ登録・ルックアップ |
| `EventStreamEvent::Extension` | 拡張ポイント付き EventStream |
| `ActorFuture` / `AskResponse` | tick ベースの非同期完了監視 |
| `SystemQueue` (CAS ベース) | ロックフリーなシステムメッセージキュー |
| `RemoteAuthorityResolver` | リモートアクター参照の解決 |
| `TransportInformation` | リモーティング用トランスポート情報 |
| `ChildRef<M, TB>` | 型付き子アクター参照 |
| `BehaviorRunner` | Behavior の実行ランナー |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `MessageAdaptionFailure` シグナル — `BehaviorSignal` に新バリアント追加のみ

### Phase 2: easy（単純な新規実装）

- `ChildFailed` シグナル — SupervisorStrategy 処理パスからシグナルを発行
- `PreRestart` シグナル — リスタート前のライフサイクルフックをシグナルとして公開

### Phase 3: medium（中程度の実装工数）

- `Routers.pool(size)` — 固定サイズプールルーター。子アクター生成 + ラウンドロビン

### Phase 4: hard（アーキテクチャ変更を伴う）

- `Receptionist` / `ServiceKey` — サービスディスカバリ。分散レジストリ基盤が必要
- `Routers.group(serviceKey)` — Receptionist に依存するグループルーター

### 対象外（n/a）

- `ClassicActorSystemProvider` — Classic actor 互換。fraktor-rs は独自の Untyped 実装を持つ
- `ActorTestKit` 内部実装 — JVM テストフレームワーク固有
- `LoggerOps` / `LogMessages` — JVM ロギングフレームワーク固有
- `DispatcherSelector` — JVM スレッドプール固有。fraktor-rs は TickDriver ベース

---

## 総評

fraktor-rs の actor モジュールは **Pekko Typed Actor の中核 API をほぼ完全に網羅**しており、カバレッジは前回の ~85% から **~92%** に向上した。特に、タイマー（`TimerScheduler` / `withTimers`）、ビヘイビアインターセプタ（`BehaviorInterceptor`）、アイドルタイムアウト（`setReceiveTimeout`）、カスタム終了メッセージ（`watchWith`）が新たに実装され、6件のギャップが解消された。

残るギャップは以下の 2 領域に集中：

1. **サービスディスカバリ**（Receptionist, ServiceKey）— 分散レジストリ基盤が必要
2. **ルーティング**（Pool/Group Router）— Receptionist 依存のグループルーターを含む

コア機能（Behavior, Supervision, StashBuffer, Ask, Extension, Timer, Interceptor, ReceiveTimeout, WatchWith）は完全にカバーされており、**Pekko Typed Actor を使った一般的なアクターパターンは fraktor-rs でそのまま実現可能**。

ギャップの大半は「便利機能」であり、コアのアクターモデルには影響しない。YAGNI 原則に従い、Phase 1-2 の trivial/easy 項目を優先実装し、Receptionist 等は cluster モジュールとの統合時に検討するのが妥当。
