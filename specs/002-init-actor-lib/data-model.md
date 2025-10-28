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
- **検証ルール**: `Bounded.capacity > 0`。`Grow.max_factor >= 1.0`。`overflow == Block` の場合は `DispatcherConfig.mode == DispatchMode::HostAsync` であり、`MailboxRuntime` が `AsyncQueue` バックエンドを選択していることを構成時に検証する。

## 6. MailboxRuntime<M>
- **役割**: メールボックス実行体。`SyncQueue`/`AsyncQueue` をラップし、Dispatcher へのスケジューリングを行う。ユーザーメッセージとシステムメッセージを別キューで管理し、Suspend/Resume により処理を制御する。
- **フィールド**:
  - `user_queue_backend: MailboxBackend<Envelope<M>>` (`SyncQueue`/`AsyncQueue` 双方を共通の同期インターフェイスでラップ)
  - `system_queue_backend: MailboxBackend<SystemMailboxEnvelope>` (常に優先処理対象。`OverflowPolicy::Block` は HostAsync + `AsyncQueue` のみ)
  - `status: Shared<MailboxStatus>` (`Running | Suspended { reason } | Draining | Closed`)
  - `suspend_state: Shared<SuspendState>` (`Active | Paused { at: Instant }`)
  - `ready_queue_link: ReadyQueueLink` (DispatcherRuntime への再登録フック)
  - `metrics: ObservationChannel<MailboxMetric>`
  - `middleware_chain: MiddlewareChain<M>`
  - `stash: StashBuffer<M>`
- **状態遷移**: `Running -> Suspended` (Dispatcher からの指示または backpressure) → `Running` (resume) / `Draining` (stop) → `Closed`。`Closed` は終端。Suspend 中も `system_queue_backend` のメッセージは処理可能とし、Resume 時には `ready_queue_link` を通じて再スケジュールを実行する。

## 7. DispatcherConfig
- **役割**: メールボックス処理スケジューラの設定。
- **フィールド**:
  - `name: DispatcherName`
  - `throughput: u16`
  - `fairness: FairnessStrategy` (`RoundRobin | WorkStealing`)
  - `worker_budget: Duration`
  - `mode: DispatchMode` (`CoreSync | HostAsync`)
- **検証ルール**: `throughput > 0`。`mode == HostAsync` の場合は `modules/actor-std` 側でアダプタを必須とする。

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

## 15. DispatcherRuntime
- **役割**: `DispatcherConfig` に従ってワーカースレッド／タスクを管理し、各 MailboxRuntime に `MessageInvoker` を割り当ててスケジュールする。protoactor-go の `dispatcher/process_registry.go` と Apache Pekko の `Dispatcher` が提供する公平性/スループット制御を Rust 向けに再構成する。
- **フィールド**:
  - `config: DispatcherConfig`
  - `worker_pool: Shared<WorkerPool>` (`WorkerId` ごとの状態遷移を保持、少なくとも 2 ワーカー以上を確保)
  - `queue_selector: MailboxSelector` (`Priority`, `Balanced`, `Dedicated`)
  - `metrics: ObservationChannel<DispatcherMetric>`
  - `clock: DispatcherClock` (スケジューリング間隔計測用抽象)
- **検証ルール**: `worker_pool` のサイズは `config.throughput` と一致する必要はないが、`throughput == 0` は許容しない。常に 2 スレッド以上のワーカーを確保し、スレッドプールを構成できない環境では DispatcherRuntime を起動してはならない。`config.mode == DispatchMode::HostAsync` の場合、各ワーカーは `Future` をポーリングできるエグゼキュータを所有していなければならない。
- **状態遷移**: `Starting -> Running -> Draining -> Stopped`。`Draining` 中は新規 Mailbox 登録を拒否し、既存ワークを完了後 `Stopped` へ遷移。

## 16. MessageInvoker<M>
- **役割**: `MailboxRuntime` からシステム／ユーザーメッセージを取り出し、`BehaviorProfile` の `next`/`post_stop` を実行する協調的エグゼキュータ。Suspend/Resume や backpressure ヒントを DispatcherRuntime と MailboxRuntime の双方に反映させる。
- **フィールド**:
  - `mailbox_shared: ArcShared<MailboxRuntime<M>>`
  - `behavior: BehaviorProfile<M>`
  - `observation: ObservationChannel<InvokerMetric>`
  - `context_factory: ContextFactory`
  - `dead_letters: ObservationChannel<DeadLetter>`
- **検証ルール**: `mailbox_shared` から取得するメッセージ順序はシステムキューが常に先、次にユーザーキュー。Suspend 中はユーザーキューを取得しない。`InvokerMetric::loop_latency` が `config.worker_budget` を超える場合、DispatcherRuntime へ backpressure ヒントを返す。

## 17. ReadyQueueLink
- **役割**: DispatcherRuntime へ mailbox の再登録・解除・ヒント送出を行う接続子。ReadyQueueCoordinator の抽象を隠蔽し、MailboxRuntime が依存し過ぎないようにする。 
- **フィールド**:
  - `coordinator: Shared<ReadyQueueCoordinator>`
  - `mailbox_id: MailboxId`
  - `last_hint: ThroughputHint`
- **検証ルール**: `mailbox_id` はスコープ内で一意。`notify_ready` 呼び出し時には重複登録を防ぐ。Backpressure ヒント送出時は最新ヒントとの差分が一定閾値を超えた場合のみ通知し、過剰なシグナルを抑制する。

## 18. MailboxMiddlewareChain<M>
- **役割**: メッセージ処理前後に追加処理を挿入するチェイン。監査ログ、トレーシング、メッセージ変換などを既存コードに影響なく注入する。 
- **フィールド**:
  - `before: SmallVec<[MiddlewareFn<M>; 4]>`
  - `after: SmallVec<[MiddlewareFn<M>; 4]>`
  - `error: Option<MiddlewareErrorFn<M>>`
- **検証ルール**: `before`/`after` は最大長制限を持ち、超過時は構成エラー。チェインが空の場合はゼロコストでスキップされることを保証する。

## 19. MailboxMetric / InvokerMetric / DispatcherMetric
- **役割**: メールボックスと DispatcherRuntime が発火する観測イベント。投入件数、ドロップ件数、Suspend Duration、system 予約枠使用率、Invoker ループレイテンシなどを記録する。 
- **フィールド**:
  - `MailboxMetric` (`kind: MailboxMetricKind`, `value: MetricValue`, `timestamp: Ticks`)
  - `InvokerMetric` (`loop_latency: Duration`, `processed: u16`, `backpressure: Option<BackpressureHint>`)
  - `DispatcherMetric` (`fairness_score: f32`, `active_workers: u16`, `queue_depth: u16`)
- **検証ルール**: すべてのメトリクスには単調増加の `timestamp` を付与。no_std 環境では抽象化したクロックから取得し、ホスト環境との整合性を維持する。

## 20. StashBuffer<M>
- **役割**: 条件付きで保留したメッセージを維持し、再投入順序を保証するバッファ。`ActorContext` と `MailboxRuntime` の両方から操作可能。
- **フィールド**:
  - `capacity: usize`
  - `buffer: VecDeque<Envelope<M>>`
  - `policy: StashPolicy` (`DropOldest | Error`)
- **検証ルール**: 容量超過時は `StashPolicy` に従い、`Error` の場合は `ActorError::Overflow` を返す。再投入は FIFO で行い、システムメッセージは Stash しない。

## 21. ReadyQueueCoordinator
- **役割**: DispatcherRuntime のワーカープールと各 MailboxRuntime との間で、再スケジュール要求・解除・スループットヒントの調停を行う。protoactor-go の `dispatcher/process_registry` が担う ready キューの役割に相当。
- **フィールド**:
  - `ready_queue: Shared<ReadyQueue>` (MailboxId のキュー)
  - `worker_assignments: Shared<WorkerAssignments>`
  - `metrics: ObservationChannel<DispatcherMetric>`
- **検証ルール**: `ready_queue` は FIFO を維持し、同一 MailboxId が重複登録された場合でも単一エントリになるよう調整する。スループットヒントを受け取った際は `worker_assignments` を更新して DispatcherRuntime へ通知する。

## 22. ExecutionRuntimeRegistry
- **役割**: 利用可能な ExecutionRuntime を保持し、ActorSystem 起動時に適切なランタイムを DispatcherRuntime と ReadyQueueCoordinator へ提供する。
- **フィールド**:
  - `default_plugin: PluginId`
  - `plugins: BTreeMap<PluginId, ExecutionRuntimeMetadata>`
  - `state: Shared<RegistryState>`
- **検証ルール**: `default_plugin` は常に `plugins` 内に存在する。登録解除時に既定ランタイムが消失しないようロックで保護する。

## 23. ExecutionRuntime
- **役割**: DispatcherRuntime/ReadyQueueCoordinator/ワーカープールを構築・駆動するプラガブルな実行コンポーネント。CoreSync/HostAsync などのモード差を吸収する。
- **フィールド**:
  - `id: PluginId`
  - `mode: DispatchMode`
  - `worker_builder: WorkerBuilder`
  - `lifecycle: PluginLifecycleHooks`
- **検証ルール**: `worker_builder` は `mode` と整合する実装（同期/非同期）を提供する。`mode == DispatchMode::CoreSync` の場合は no_std 互換 API のみ使用する。

## リレーション概要
- `ActorSystemScope` は `BehaviorProfile` を取り込み、`MailboxRuntime`・`DispatcherRuntime`・`EventStreamCore` を生成する。
- `BehaviorProfile` は `SupervisionStrategy`, `MessageQueuePolicy`, `DispatcherConfig` を参照。
- `MailboxRuntime` は `DispatcherRuntime` と `MessageInvoker` を通じて実行され、`MessageQueuePolicy` 派生設定を共有し `ObservationChannel` へメトリクスを送出する。`ReadyQueueCoordinator` と `ReadyQueueLink` が両者のシグナリング境界として機能する。
- `ExecutionRuntimeRegistry` は `ExecutionRuntime` を保持し、DispatcherRuntime/ReadyQueueCoordinator の初期化・終了を仲介する。
- `MailboxMiddlewareChain` は `MessageInvoker` の処理前後で呼び出され、観測データは `MailboxMetric`/`InvokerMetric` に蓄積される。`StashBuffer` は `ActorContext` と `MailboxRuntime` の双方からアクセスされる。
- `SupervisionStrategy` と `ActorError` は `RestartStatistics` を更新し、`ObservationChannel<SupervisionMetric>` に情報を通知する。

## 検証・不変条件
1. すべての `Shared` 系フィールド名は末尾に `_shared` を付ける (`dispatcher_registry_shared` 等)。
2. `ActorRef` は `'scope` ライフタイムが生存中のみ `tell` を許可。`scope` Drop 後は `ScopeClosed` エラーを返す。
3. Mailbox の `overflow` が `Block` の場合、`DispatcherConfig.mode` は必ず `HostAsync` であり、`MailboxBackend` が `AsyncQueue` を通じて待機を `Future` 化する。同時に backpressure ヒントを ObservationChannel 経由で公開しなければならない。
4. Suspend 中は `system_queue_backend` のメッセージのみ処理でき、`user_queue_backend` は再開まで滞留する。Resume 直後はシステムキューを優先して空にした上でユーザーキュー処理へ戻る。
5. DispatcherRuntime は `MessageInvoker` ごとのイベントループ遅延とスループットを計測し、`ObservationChannel<DispatcherMetric>` を通じて公平性メトリクスを公開しなければならない。
6. ExecutionRuntimeRegistry は常に少なくとも 1 つの ExecutionRuntime を保持し、デフォルトランタイムが未設定の状態で ActorSystem を起動してはならない。
7. ReadyQueueCoordinator/ReadyQueueLink は再登録の冪等性を保証し、重複通知時には単一のスケジューラエントリのみが有効になる。
8. StashBuffer は system メッセージを保持しない。ユーザーメッセージのみが格納され、再投入時に FIFO 順を崩さない。
9. EventStream の購読者登録は `ScopeState::Open` 中のみ許可。購読解除は自動的に `Active -> PendingRemoval -> Removed` のステートマシンで処理する。
10. `ActorError::kind == Fatal` のとき、Supervision 再起動は禁止し `ScopeMetric::fatal_stop` を増やす。
9. `ActorError::kind == Fatal` のとき、Supervision 再起動は禁止し `ScopeMetric::fatal_stop` を増やす。
