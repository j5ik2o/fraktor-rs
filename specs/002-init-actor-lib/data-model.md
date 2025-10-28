# Data Model: Cellactor Actor Core 初期実装

## 1. ActorSystemScope
- **役割**: ActorSystem 実行セッションの境界を表し、`ActorRef` 生成・監査・シャットダウンを制御する。`Drop` 時に登録ミドルウェアとイベントストリームを閉じる。 
- **フィールド**:
  - `system_id: SystemId` (`u64` ベース、決定的カウンタ) 
  - `scope_id: ScopeId` (`u32`、スコープ毎の連番)
  - `dispatcher_registry: Shared<DispatcherRegistry>` (`Shared` 命名規約順守)
  - `adapter_registry: Shared<MessageAdapterRegistry>`
  - `observation_channel: ObservationChannel<ScopeMetric>`
  - `state: ScopeState` (`Open | Closing | Closed`)
- **リレーション**: `ActorSystemScope` → (1..n) `ActorRef<M>`、 (1) `EventStream`
- **検証ルール**: `state == ScopeState::Open` のときのみ `spawn` を許可。`Closed` で `spawn` を呼ぶと `ScopeError::Closed`。

## 2. ActorRef<'scope, M>
- **役割**: スコープ内でメッセージ送信を許可する型付き参照。ライフタイム `'scope` でスコープ外移動を禁止。
- **フィールド**:
  - `pid: ActorPid` (`u64` ベース)
  - `mailbox_shared: ArcShared<MailboxRuntime<M>>`
  - `lifecycle_shared: Shared<ActorLifecycle>`
  - `marker: PhantomData<&'scope ()>`
- **リレーション**: 属する `ActorSystemScope` によって生成; `MailboxRuntime<M>` へ参照。
- **検証ルール**: `Send` は必要最小限 (`M: Send`) を要し、`Sync` はデフォルト付与しない。`mailbox_shared` は `ScopeState::Closed` で `try_send` を失敗させる。

## 3. ActorContext<'scope, M>
- **役割**: ハンドラ実行時に提供される API。`ActorRef` 取得、スケジューリング、監視イベント発火を担う。
- **フィールド**:
  - `self_ref: ActorRef<'scope, M>`
  - `system: &'scope ActorSystemScope`
  - `stash_buffer: StashBuffer<M>`
  - `timers: TimerDriver` (no_std 抽象)
- **検証ルール**: `stash_buffer` は容量 `MessageQueuePolicy::stash_capacity` を超えた場合 `ActorError::Overflow` を返す。

## 4. BehaviorProfile<M>
- **役割**: アクター振る舞い・初期化・監視ポリシー・メールボックス/Dispatcher 設定を束ねるビルダ。
- **フィールド**:
  - `init: fn(&mut ActorContext<M>) -> InitialState<M>`
  - `next: BehaviorFn<M>` (`type BehaviorFn<M> = fn(&mut ActorContext<M>, M) -> ActorResult<M>`)
  - `post_stop: Option<fn(&mut ActorContext<M>)>`
  - `mailbox_policy: MessageQueuePolicy`
  - `dispatcher: DispatcherSelector`
  - `supervision: SupervisionStrategy`
  - `metrics: ScopeMetricConfig`
- **検証ルール**: `init` が `ActorResult::Err` を返した場合は spawn を失敗させる (再試行不可)。

## 5. MessageQueuePolicy
- **役割**: メールボックスと EventStream 内部キューの共通設定。
- **フィールド**:
  - `kind: QueueKind` (`Bounded { capacity } | Unbounded | Priority { comparator }`)
  - `overflow: OverflowPolicy` (`DropNewest | DropOldest | Grow { max_factor } | Block`)
  - `system_priority_ratio: u8` (0..=100)
  - `stash_capacity: usize`
- **検証ルール**: `Bounded.capacity > 0`。`Grow.max_factor >= 1.0`。

## 6. MailboxRuntime<M>
- **役割**: メールボックス実行体。`SyncQueue`/`AsyncQueue` をラップし、Dispatcher へのスケジューリングを行う。
- **フィールド**:
  - `inbox: ArcShared<SyncQueue<Envelope<M>, QueueKey, Backend>>`
  - `status: Shared<MailboxStatus>` (`Running | Suspended { reason } | Draining | Closed`)
  - `metrics: ObservationChannel<MailboxMetric>`
  - `middleware_chain: MiddlewareChain<M>`
- **状態遷移**: `Running -> Suspended` (backpressure/command) → `Running` (resume) / `Draining` (stop) → `Closed`。`Closed` は終端。

## 7. DispatcherConfig
- **役割**: メールボックス処理スケジューラの設定。
- **フィールド**:
  - `name: DispatcherName`
  - `throughput: u16`
  - `fairness: FairnessStrategy` (`RoundRobin | WorkStealing`)
  - `worker_budget: Duration`
  - `runtime: DispatchRuntime` (`CoreSync | HostAsync`)
- **検証ルール**: `throughput > 0`。`runtime == HostAsync` の場合は `modules/actor-std` 側でアダプタを必須とする。

## 8. ActorError
- **役割**: Supervision 判定で利用するエラー構造体。
- **フィールド**:
  - `kind: ActorErrorKind` (`Transient | Permanent | Fatal`)
  - `retry: RetryPolicy` (`Attempts { max, within } | None`)
  - `severity: Severity` (`Info | Warn | Error`)
  - `labels: SmallVec<[u8; 24]>` (no_std 対応のタグ格納)
  - `source: Option<ErrorSource>` (panic 等)
- **検証ルール**: `Transient` の場合は `retry` が `Attempts` を要求。`Fatal` は常に `RetryPolicy::None`。

## 9. SupervisionStrategy
- **役割**: `ActorError` を入力に再起動・停止などを決定。
- **フィールド**:
  - `decider: fn(&ActorError, &RestartStatistics) -> SupervisionDecision`
  - `restart_limit: RestartLimit` (`max_restarts`, `within_duration`)
  - `children_policy: SupervisionMode` (`OneForOne | AllForOne`)
  - `metrics: ObservationChannel<SupervisionMetric>`
- **検証ルール**: `restart_limit.max_restarts > 0` の場合、`within_duration > 0` 必須。`SupervisionDecision::Resume` は `Transient` のみ許可。

## 10. EventStreamCore
- **役割**: publish/subscribe, backpressure, 観測イベントを担当。
- **フィールド**:
  - `queue: ArcShared<SyncQueue<EventEnvelope, MpscKey, Backend>>`
  - `subscriptions: Shared<SubscriptionRegistry>`
  - `metrics: ObservationChannel<EventStreamMetric>`
  - `backpressure: BackpressureConfig`
- **状態遷移**: `Active -> Backpressured` (ドロップ/遅延指示) → `Active`。`Closed` で購読不可。

## 11. ObservationChannel<T>
- **役割**: メトリクス・イベント通知を push ベースで流す抽象。
- **フィールド**:
  - `sender: Shared<SyncQueue<T, MpscKey, Backend>>`
  - `listeners: Shared<ListenerRegistry>`
  - `mode: ObservationMode` (`Immediate | Buffered { capacity }`)
- **検証ルール**: `Buffered.capacity > 0`。`listeners` が空の状態で `mode == Immediate` のときはイベントを `DeadObservation` として記録。

## 12. MessageAdapterRegistry
- **役割**: 型変換アダプタの登録地点。型消去メッセージ (`ErasedMessageEnvelope`) と型付きハンドラを橋渡し。
- **フィールド**:
  - `adapters: Shared<BTreeMap<TypeId, AdapterEntry>>`
  - `dead_letters: ObservationChannel<DeadLetter>`
- **検証ルール**: 同一 `TypeId` で複数登録された場合は最新エントリで上書きし、警告ログと観測イベントを発火。

## 13. ErasedMessageEnvelope
- **役割**: 型消去メッセージと送信者 PID を保持する内部コンテナ。`Untyped` 命名を回避。
- **フィールド**:
  - `payload: SharedAny` (`Shared<dyn Any + Send>` equivalent via erased wrapper)
  - `sender_pid: ActorPid`
  - `headers: SmallVec<[HeaderEntry; 8]>`
- **検証ルール**: `payload` 取り出しは登録アダプタが存在する場合のみ成功し、失敗時は Dead Letter に転送。

## 14. RestartStatistics
- **役割**: Supervision 判定に必要な統計値を保持。
- **フィールド**:
  - `failures: u32`
  - `window_start: Instant` (utils-core 時間抽象の `Ticks`)
  - `last_error: Option<ActorErrorKind>`
- **検証ルール**: `window_start` から `within_duration` を超えた場合は `failures` を 0 にリセット。

## リレーション概要
- `ActorSystemScope` は `BehaviorProfile` を取り込み、`MailboxRuntime` と `EventStreamCore` を生成する。
- `BehaviorProfile` は `SupervisionStrategy`, `MessageQueuePolicy`, `DispatcherConfig` を参照。
- `MailboxRuntime` と `EventStreamCore` は共通の `MessageQueuePolicy` 派生設定を共有し、`ObservationChannel` へメトリクスを送出する。
- `SupervisionStrategy` と `ActorError` は `RestartStatistics` を更新し、`ObservationChannel<SupervisionMetric>` に情報を通知する。

## 検証・不変条件
1. すべての `Shared` 系フィールド名は末尾に `_shared` を付ける (`dispatcher_registry_shared` 等)。
2. `ActorRef` は `'scope` ライフタイムが生存中のみ `tell` を許可。`scope` Drop 後は `ScopeClosed` エラーを返す。
3. Mailbox の `overflow` が `Block` の場合、Dispatcher は backpressure ヒントを ObservationChannel 経由で公開しなければならない。
4. EventStream の購読者登録は `ScopeState::Open` 中のみ許可。購読解除は自動的に `Active -> PendingRemoval -> Removed` のステートマシンで処理する。
5. `ActorError::kind == Fatal` のとき、Supervision 再起動は禁止し `ScopeMetric::fatal_stop` を増やす。
