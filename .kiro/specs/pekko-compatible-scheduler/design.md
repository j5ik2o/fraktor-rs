# Design Document

## Overview
本機能は Pekko 互換の Scheduler を fraktor-rs に導入し、ActorSystem 全体に deterministic なタイマー／周期実行基盤を提供する。ディスパッチャや mailbox、Remoting 等の内部機能が共通 Scheduler を介して遅延・周期処理を行えるようになり、Pekko と同じ API 契約（scheduleOnce／scheduleAtFixedRate 等）を Rust/no_std 環境で再現する。
利用者は ActorSystem 構築時に Scheduler を自動的に取得し、DelayProvider や system mailbox からの遅延実行が一貫したタイマー精度・観測性を得られる。導入により、従来個別実装だった DelayFuture も Scheduler 上の単発タイマーに集約され、診断や統計取得も統一される。

### Goals
- Pekko 互換 API（scheduleOnce/fixedRate/fixedDelay/cancelable）を ActorSystem に実装
- RuntimeToolbox 経由で monotonic clock と timer wheel を差し替え可能にする
- observability（EventStream, metrics, diagnostics）とテスト用仮想クロックを提供

### Non-Goals
- Quartz など長期ジョブスケジューラとの統合
- エンジン外部の cron 連携や分散スケジューラ機能
- Remoting プロトコルの具体実装（別 spec で扱う）

## Architecture

### Existing Architecture Analysis
- 依存方向: `utils-core → actor-core → actor-std` を維持。タイマー/クロックの Primitive は utils-core で提供し、actor-core は RuntimeToolbox を通じて利用する。
- 既存 DelayProvider/DelayFuture は mailbox/queue timeout で広く使用されており、破壊的変更を避けるため Scheduler ベースの実装に裏側のみ差し替える。
- EventStream / system mailbox など既存観測パスを再利用し、Scheduler から直接アクターへメッセージを push しない。

### High-Level Architecture
```mermaid
graph TB
  subgraph utils_core
    clock[MonotonicClock]
    timer[TimerWheelFamily]
  end
  subgraph actor_core
  scheduler[Scheduler]
  cancellable[CancellableRegistry]
  actor_system[ActorSystemBuilder]
  delay_provider[SchedulerBackedDelayProvider]
  runner[SchedulerRunner]
  end
  subgraph actor_std
    host_timer[StdTimerImpl]
  end
  clock --> scheduler
  timer --> scheduler
  scheduler --> runner
  scheduler --> cancellable
  scheduler --> delay_provider
  actor_system --> scheduler
  actor_system --> runner
  host_timer --> timer
```
**Architecture Integration**
- 既存パターン: RuntimeToolbox を介した抽象、system mailbox 経由配送、EventStream ベース監視。
- 新規コンポーネント: `MonotonicClock`, `TimerWheelFamily`, `Scheduler`, `SchedulerRunner`, `SchedulerBackedDelayProvider`, `CancellableHandle`。
- 技術整合: utils-core に no_std 互換の TimerWheel と clock を実装し、std 実装は actor-std で `Instant::now()` を利用。
- `ToolboxTickSource` 抽象を RuntimeToolbox が提供し、tokio/embassy/SysTick などの実装詳細は utils-core（no_std）/actor-std の側へ完全に閉じ込める。actor-core からは `TickSource::try_pull()` で取得できる `TickEvent` のみを扱い、割り込みや async runtime の分岐は一切存在しない。
- Mailbox と同様の event loop パターンを SchedulerRunner が流用し、tick 供給は `TickSource` から pull するだけで済むため、no_std 環境でも `cfg(feature = "std")` を追加せずに済む。
- 階層分離: actor-core の責務は `TickEvent` を消費して TimerWheel を進め、SystemMailbox へ配送することに限定する。tokio task や ISR ハンドラの登録・解除は Toolbox 実装の責務となり、ステアリングで定義された no_std ファースト方針と完全に整合する。

### Technology Stack and Design Decisions
- `utils-core/time`: `MonotonicClock`（tick 64bit, resolution 指定）, `TimerInstant`, `TimerWheel`（固定長スロット + overflow priority queue）、`TimerEntryMode`。
- `RuntimeToolbox` 拡張: `type Clock`, `type Timer`, `fn clock(&self)`, `fn timer(&self)` に加えて `fn tick_source(&self) -> &'static dyn ToolboxTickSource` を追加する。Toolbox 側で tokio/embassy/SysTick などの割り込み登録・停止を完結させ、actor-core には `TickEvent { pending_ticks: u16, mode: TickMode }` を pull する API だけを公開する。NoStdToolbox は SysTick/DWT/embassy の ISR から `pending_ticks` をインクリメントする実装、actor-std は tokio タスクから同一インタフェースでイベントを push する実装を提供する。

#### ToolboxTickSource Contract
- **イベントセマンティクス**: `ToolboxTickSource` は ISR/async task から呼ばれる `notify_tick()` で `pending_ticks` カウンタを単にインクリメントするだけとし、deadline 計算や `SmallVec` 生成は一切行わない。`SchedulerRunner` は `tick_source.try_pull()` を呼び、`TickEvent { pending_ticks, mode }` を受け取ったら自身で `deadline = last_deadline + resolution` を繰り返し計算して `run_tick(deadline)` を実行する。これにより ISR の処理時間は O(1) に固定され、no_std 環境でも割り込み遅延を一定に抑えられる。
- **Flush/Stop**: `ToolboxTickSource::stop_and_flush()` は (1) 割り込み/タスク登録を解除し新規 `notify_tick()` を遮断、(2) `pending_ticks` を原子的に読み取って `TickEvent::from_pending()` を返すだけで済む。残 deadline の算出は Runner 側が一括で行うため determinism を維持したまま簡潔に停止できる。
- **Backpressure Reporting**: `tick_source.status()` は `TickSourceStatus::{Ok, Backpressured { pending_ticks }, Suspended}` を返し、Runner は status 変化を `SchedulerDiagnostics::record_driver_status()` へ通知する。Toolbox 実装は queue/critical-section 等の内部情報を保持するが、actor-core は状態 enum のみを観測する。
- Actor-side: `Scheduler<TB>` が timer wheel を poll して `SchedulerCommand` を処理、`SchedulerHandle` が `Cancellable` として利用可能。`SchedulerRunner` は `RunnerMode::{Manual, AsyncHost, Hardware}` を実装し、いずれのモードでも tick 取得は `TickSource` 経由で統一。バックプレッシャ変化を検知したら (a) backlog deadlines を全て `run_tick` で消化し、(b) `SchedulerBackpressureLink::raise(Driver)` を呼んで accepting_state を Backpressure へ遷移させる。pending が閾値を下回れば `SchedulerBackpressureLink::clear(Driver)` で通常状態へ復帰させる。

##### ToolboxTickSource Implementation Notes
- **TickCounter**: `ToolboxTickSource` は `pending_ticks: AtomicU16` と `last_tick: TimerInstant` のみを保持し、`notify_tick()` は `pending_ticks.fetch_add(1, Ordering::AcqRel)` で終了する。ISR 内で `SmallVec` を確保せず、スタック使用量と実行時間を一定に保つ。
- **Catch-up Pull**: Runner は `tick_source.try_pull()` から `TickEvent { pending_ticks, mode }` を受け取り、その場で `pending_ticks` 回ぶんの deadline を計算する。`mode == CatchUp` は backlog がソフト閾値（例: 0.8 * max_pending）を超えたことを示すだけで、deadline 展開は Runner が行う。
- **Backpressure Hook**: queue/critical-section が飽和して `notify_tick()` が `Backpressure` を返した場合でも、driver 側は pending を破棄しない。Runner は `TickSourceStatus::Backpressured` を受信した時点で `SchedulerBackpressureLink::raise(Driver)` を呼び、タスク受理ポリシーを切り替える。
- **ISR/Thread 安全性**: no_std モードでは `notify_tick()` が割り込みから呼ばれるため、`pending_ticks` 加算のみの実装で最小限のクリティカルセクションに収める。std モードでは tokio タスクが `notify_tick()` を単純呼び出しするだけで済み、両者が同じ `TickSource` API を共有する。
- **テストハーネス**: ManualClock/Deterministic モードでは `tick_source.inject_manual_tick(n)` を提供し、Runner が同じコードパスで catch-up を実行できるようにする。

###### Catch-up Backlog Configuration
- **容量計算**: `SchedulerConfig::catch_up_window_ms`（デフォルト 50ms）と `resolution_ns` から `max_pending_ticks = min(64, (catch_up_window_ms * 1_000_000) / resolution_ns)` を算出し、`TickSource` の `pending_ticks` がこの上限を超えないよう clamp する。例えば resolution=10ms, window=50ms なら `max_pending_ticks = 5` と算出される。
- **Catch-up イベント**: backlog が `0.8 * max_pending` を超えると `TickEvent.mode = CatchUp` をセットし、Runner が受信直後に `SchedulerDiagnostics::record_catch_up(pending_ticks)` を呼ぶ。Runner は `pending_ticks` 回ぶんの deadline を順次計算して `run_tick` を実行し、±1 tick 以内で補償する。
- **Backpressure 連携**: `pending_ticks` が上限を超えた状態で `notify_tick()` が `Backpressure` を返した場合でも、driver 側は tick を破棄しない。Runner は `TickSourceStatus::Backpressured` を受信した時点で `SchedulerBackpressureLink::raise(Driver)` を呼び、新規タイマー受理ポリシーを切り替える。
- **診断項目**: `SchedulerDiagnostics` に `tick_backlog_peak`, `tick_overflow_events` を追加し、backlog 設定値が適切かどうかを観測できるようにする。
- **飽和ポリシー**: `pending_ticks > max_pending_ticks` が連続 2 回観測された場合、Runner は `SchedulerWarning::DriverCatchUpExceeded { pending_ticks }` を EventStream/Diagnostics へ送出し、`accepting_state = Rejecting` へ遷移して低優先度タイマーを fail-fast する。`pending_ticks` が `max_pending_ticks / 2` 未満に戻ったら `SchedulerBackpressureLink::clear(Driver)` を呼び段階的に Normal へ戻す。
- **構成ガイド**: `MAX_PENDING = 64` の根拠として、`resolution_ns` と `worst_case_jitter_ms` を用いた計算式（例: Cortex-M で 1kHz SysTick、最悪停止 3ms ⇒ `max_pending_ticks = ceil(3ms / resolution)`）を Performance セクションに記載する。`SchedulerConfig::validate()` はハードウェア種別ごとの推奨上限表と照合し、超過した設定に対して `SchedulerConfigError::CatchUpWindowTooLarge` を返す。

## Deterministic Execution Guarantees
- **FIFO Preservation**: `Scheduler::schedule_*` は `TimerCommandQueue`（lock-free `ToolboxMpscQueue` + 単調増加する `sequence_id`）に投入し、`run_tick` は `sequence_id` 昇順に `TimerWheel` へ登録する。各スロットは `SlotQueue`（固定長 `ArrayQueue`）で構成し、同一 tick で満期になったエントリは挿入順にデキューされることで Requirement 1 AC2 を満たす。
- **Tick Drift Budget**: `TimerWheelConfig` が `resolution_ns` と `drift_budget_pct`（既定 5%）を保持し、`SchedulerDiagnostics::drift_monitor` が `deadline` と `now` の差分を監視する。許容ドリフトを超過すると `SchedulerWarning::DriftExceeded { observed_ns }` を EventStream と診断ストリームへ送信し、R1 AC3 の決定的挙動を担保する。
- **max_frequency API**: `Scheduler::max_frequency()` は `Hertz::from_nanos(resolution_ns)` を返し、ActorSystemBuilder 経由で DelayProvider や subsystem へ共有する。`RuntimeToolbox` 実装は `TimerWheelConfig` を提供し、std/no_std 間で同一の上限値を保証する。
- **ManualClock Integration**: 決定論テストでは `ManualClock::advance(n)` が `run_tick` を同期実行し、`drift_monitor` をゼロドリフトに保つ。`deterministic_mode` はタスク ID と発火時刻をログし、Requirement 1 AC1 と Requirement 5 AC1-AC2 のリプレイ検証を実現する。
- **ClockKind 切替**: `MonotonicClock` trait に `fn kind(&self) -> ClockKind`（`Deterministic`, `RealtimeHost`, `RealtimeHardware`）を追加し、`SchedulerRunner` は `kind` に応じて `RunnerMode::Manual`（ManualClock が `advance` で tick 進行）、`RunnerMode::AsyncHost`（StdClock: tokio task が `tick_source.notify_tick()` を定期呼び出し）、`RunnerMode::Hardware`（no_std: SysTick/embassy ISR が `notify_tick()` を直接叩く）を選択する。`drift_monitor` は `Deterministic` では 0 許容、`RealtimeHost/RealtimeHardware` では `drift_budget_pct` を適用しつつ driver が報告する `DriverJitter` を差し引いて監視する。
- **Drift Compensation Loop**: `RunnerLoop` は `tick_source.try_pull()` から取得した `TickEvent { pending_ticks, mode }` を逐次消化し、各 tick ごとに `run_tick(deadline)` を呼び出す。`drift_monitor` が `observed_ns > drift_budget_pct` を返した場合でも tick を欠落させず、`pending_ticks` 回の catch-up 実行で `last_deadline` と `clock.now()` の差を 1tick 以内へ押し戻す。queue が一時的に詰まっても TickSource は deadline を保持せず pending を積むだけなので、`ExecutionBatch::missed_runs` は「Actor 側の処理遅延」に限定される。catch-up 期間中は `SchedulerDiagnostics::drift_compensations` を increment し、補償回数と Backpressure 状態を観測できるようにする。

**Key Design Decisions**
1. **Decision**: RuntimeToolbox へ clock/timer ファミリをぶら下げる抽象
   - **Context**: no_std / std 両対応かつ deterministic 動作が必要
   - **Alternatives**: (a) グローバル static タイマー、(b) DelayProvider 直接拡張、(c) Toolbox へ注入
   - **Selected**: (c) Toolbox へ `type Clock`/`type Timer`
   - **Rationale**: 依存方向を守りつつ差し替え容易。テストで ManualClock 実装が可能
   - **Trade-offs**: Toolbox API が増加、導入時に全 TB 実装の更新が必要
2. **Decision**: TimerWheel + overflow priority queue
   - **Context**: 数千単位のタイマーを no_std メモリ制約下で処理しつつ、長期遅延を扱う必要がある
   - **Alternatives**: (a) binary heap 単体, (b) hierarchical wheel, (c) 固定長 wheel + overflow priority queue
   - **Selected**: (c)
   - **Rationale**: 近傍期限は O(1) で処理し、wheel 範囲外は `BinaryHeap` に蓄積し threshold に達したら wheel へ再挿入することでドリフトを抑える
   - **Trade-offs**: 再挿入時に heap 操作コストが発生するが対象は遠未来タイマーに限定される
3. **Decision**: DelayProvider を Scheduler facade に置き換え
   - **Context**: 既存 mailbox timeout の互換性
   - **Alternatives**: (a) DelayProvider 廃止、(b) DelayProvider を scheduler wrapper、(c) DelayProvider を scheduler とは独立
   - **Selected**: (b)
   - **Rationale**: 既存 API を維持しながら実装を集約
   - **Trade-offs**: scheduler 実装が DelayProvider の追加要件を意識する必要がある

## System Flows
### タイマー登録と発火
```mermaid
sequenceDiagram
  participant Caller
  participant Scheduler
  participant Runner
  participant TimerWheel
  participant SystemMailbox
  Caller->>Scheduler: schedule_once(delay, handler)
  Scheduler->>TimerWheel: insert(entry)
  Runner->>Scheduler: run_tick()
  Scheduler->>TimerWheel: advance(clock.now)
  TimerWheel-->>Scheduler: expired entries
  Scheduler->>SystemMailbox: enqueue(handler)
  SystemMailbox-->>Handler: deliver()
```

### Shutdown/TaskRunOnClose
```mermaid
sequenceDiagram
  participant ActorSystem
  participant Scheduler
  participant TickSource
  participant TaskRunQueue
  participant TimerWheel
  ActorSystem->>Scheduler: shutdown()
  Scheduler->>TickSource: stop_and_flush()
  TickSource-->>Scheduler: PendingTicks(flushed)
  Scheduler->>TaskRunQueue: execute(SystemCritical→Runtime→User)
  TaskRunQueue-->>Scheduler: TaskRunOnCloseResult
  Scheduler->>TimerWheel: drain()
  TimerWheel-->>Scheduler: expired tasks (execute_or_cancel inline)
  Scheduler->>ActorSystem: report completion
```

### SchedulerRunner モード切替
- `RunnerMode::Manual`（ManualClock, kind = Deterministic）: `ActorSystem` がテスト用 RuntimeToolbox を構築すると `SchedulerRunner` はハードウェアループを持たず、`ManualClock::advance` が直接 `run_tick` を呼ぶ。`drift_monitor` はゼロドリフトを期待し、逸脱は即 `SchedulerWarning::DriftExceeded` となる。
- `RunnerMode::AsyncHost`（StdInstantClock, kind = RealtimeHost）: ActorSystemBuilder が std Toolbox を組み立てる際に tokio task を起動し、`resolution` ごとに `tick_source.notify_tick()` を呼ぶ。tokio 依存コードは actor-std 側の Toolbox 実装が保持し、actor-core は `TickSource` 抽象のみを参照する。
- `RunnerMode::Hardware`（SysTick/DWT/embassy, kind = RealtimeHardware）: RuntimeToolbox が `ToolboxTickSource` を提供し、周期割り込みは `notify_tick()` で `pending_ticks` を加算するだけ。`RunnerLoop`（通常は scheduler 専用タスク）が `tick_source.try_pull()` で pending を消費して `run_tick` を実行し、SystemMailbox との同一コンテキストを保つ。ISR では加算のみのため、R4 AC1 の並行安全性と R1 AC1 の ±1 tick 制約を維持できる。
- いずれのモードでも `SchedulerLifecycleHook` と Diagnostics の API は共通で、切り替えは `MonotonicClock::kind()` と RuntimeToolbox の driver 実装を見るだけで完結する。

## Requirements Traceability
| Requirement | Summary | Components | Interfaces | Flows |
| --- | --- | --- | --- | --- |
| R1 | ドリフト制御・上限・エラー報告 | MonotonicClock, TimerWheelConfig, Scheduler | `schedule_once`, `max_frequency`, diagnostics API | Timer登録/発火フロー |
| R2 | 周期・fixedレート・保留 | Scheduler, TimerEntryMode, ExecutionBatch, CancellableRegistry | `schedule_fixed_rate`, `schedule_fixed_delay`, `SchedulerBatchContext::current` | Timer登録/発火フロー |
| R3 | Toolbox連携・TaskRunOnClose | RuntimeToolbox, SchedulerLifecycleHook, SchedulerExecutionContext | `RuntimeToolbox::timer`, `Scheduler::shutdown`, `SchedulerExecutionContext::dispatcher` | Shutdown フロー |
| R4 | 並行安全・メトリクス | CancellableRegistry, EventStreamAdapter | `SchedulerHandle`, `emit_warning` | Timer登録/発火 |
| R5 | テスト/診断 | ManualClock, DiagnosticsRecorder, DumpFormatter | `SchedulerDiagnostics::enable`, `ManualClock::advance` | Timer登録/発火, Shutdown |

## Components and Interfaces
### utils_core/time
- **MonotonicClock**: `fn now(&self) -> TimerInstant`; 実装: `SysTickClock`, `StdInstantClock`。`TimerInstant` は `ticks: u64`, `resolution_ns: u32`。
- **TimerWheelFamily**: `fn insert(&self, entry: TimerEntry) -> TimerHandleId`, `fn cancel(&self, id)`, `fn poll_expired(&self, cx) -> Poll<ExpiredEntry>`。内部で固定長 wheel を tick で進め、範囲外のエントリは `BinaryHeap` + `SmallVec` で保持し threshold 到達時に wheel へ再挿入する。
- **TimerEntry**: `TimerEntryMode`（OneShot/FixedRate/FixedDelay）、`deadline: TimerInstant`, `payload: TimerAction`。
- **ManualClock/ManualTimer**: テスト用実装。`advance(duration)` で wheel を強制 tick。
- **TickSourceStatus**: `ToolboxTickSource` は `status()` で `TickSourceStatus::{Ok, Backpressured { pending_ticks }, Suspended}` を返す。`Backpressured` はクリティカルセクションの飽和や ISR 遅延により pending が閾値を超えたことを示し、RunnerLoop が即座に catch-up と Backpressure 遷移を行うシグナルとなる。`Suspended` は `stop_and_flush()` 実行中の状態を意味し、`Scheduler::shutdown` が TaskRunOnClose を開始しても新規 tick が入らないことを保証する。

### utils_core/delay
- **SchedulerBackedDelayProvider**: 既存 DelayProvider を実装し、`scheduler.schedule_once(duration, DelayFutureWaker)` を内部的に使用。`DelayFutureWaker` は `ExecutionBatch` を必ず受け取り、`runs` と `missed_runs` を DelayFuture の `StateMachine` へ伝搬してミス Tick 折り畳みや drift ログを生成する。
- **DelayFuture**: 変更点は handle/cancel を新 Scheduler ハンドルに委譲しつつ、`ExecutionBatch` を観測して `poll` が `missed_runs > 0` の場合に 1 回の Wake へ集約する。API は従来どおり `Future<Output = ()>` を維持し、追加情報は diagnostics へ流す。

### actor_core/scheduler
- **Scheduler<TB>**: `fn schedule_once`, `fn schedule_fixed_rate`, `fn schedule_fixed_delay`, `fn cancel(handle)`、`fn run_tick(&mut self, now: TimerInstant)`。内部に `TimerCommandQueue`, `CancellableRegistry`, `TaskRunOnCloseQueue`, `OverflowReinserter` を持つ。
- **SchedulerHandle**: `impl Cancellable`。`cancel()` は registry を通じて TimerWheel と同期。
- **SchedulerRunner**: MailboxDispatcher と同じ event-loop パターンで Scheduler を駆動。テスト時は `RunnerMode::Manual` が `ManualClock::advance` から `tick_source.inject_manual_tick` を呼ぶ。std 環境は `RunnerMode::AsyncHost` が tokio task で `tick_source.notify_tick()` を発火し、no_std/embassy は `RunnerMode::Hardware` が ISR から同 API を呼ぶ。`RunnerLoop` は `tick_source.try_pull()` で pending を取得して `run_tick` を実行する。
- **SchedulerTaskContract**: すべての内部タスクは `trait SchedulerTask { fn run(&self, batch: ExecutionBatch) -> Result<(), HandlerError>; }` を実装し、`ExecutionBatch { runs: NonZeroU32, missed_runs: u32, mode: BatchMode(OneShot|FixedRate|FixedDelay) }` を介して周期情報を共有する。公開 API は Pekko と同一だが、SystemMailbox がメッセージ/Runnable をデキューするときに `SchedulerBatchContext::push(batch, completion)` を呼び、実行完了後に Drop される `BatchGuard` が自動で pop する。guard は std では thread-local、no_std では `RuntimeToolbox::task_local_slot()` の上に構築され、割り込み安全に batch を保管する。
- **SchedulerPolicyRegistry**: ActorSystemBuilder が `SchedulerPolicyRegistry` を生成し、`SchedulerAffinity`（dispatcher, guardian, subsystem 等）単位で `PriorityProfile` と `FixedRatePolicy { backlog_limit, burst_threshold }` を登録する。公開 API (`schedule_once`, `schedule_at_fixed_rate`, など) からは追加引数を受け取らず、Registry が dispatcher/receiver から該当プロファイルを解決して `priority` や `backlog_limit` を注入する。ユーザが個別設定を行いたい場合は Builder でポリシーを上書きしつつ、Pekko 互換シグネチャは維持される。
- **SchedulerDiagnostics**: 決定論モードログ、dump 生成、EventStream 通知。
- **SchedulerLifecycleHook**: ActorSystem shutdown 時に `Scheduler::shutdown` を呼び、(1) `ToolboxTickSource::stop_and_flush()` で tick 供給を停止、(2) 残り `pending_ticks` を Runner 内で即座に消化、(3) TaskRunOnClose を優先度順に実行、(4) TimerWheel を drain した結果を Scheduler 内で直接 `run_or_cancel` し、SystemMailbox へ新規 enqueue しない。driver 停止完了までは新規 tick を `Pending` 状態で保持し、完了後に破棄して R3 AC3/AC7 を満たす。

#### TaskRunOnClose API
- **TaskRunOnClose**: `trait TaskRunOnClose { fn run(&self, batch: ExecutionBatch) -> Result<(), HandlerError>; }`。Scheduler 内部の cleanup、DelayProvider、Remoting が実装し、shutdown 中に deterministic に実行される。
- **優先度**: `TaskRunPriority`（`SystemCritical`, `Runtime`, `User`）を導入。`TaskRunOnCloseQueue` は `(priority, sequence_id)` をキーにした `BinaryHeap` で、優先度降順・同順位は登録順 FIFO を保証する。
- **登録/解除 API**: `pub fn register_on_close(&self, task: Arc<dyn TaskRunOnClose>, priority: TaskRunPriority) -> TaskRunHandle`。`SchedulerState::Active` でのみ登録を許可し、`TaskRunHandle` により `cancel_on_close(handle)` も提供する。`SchedulerState::ShuttingDown` 以降は `SchedulerError::Shutdown` を返し、新規登録を拒否する。
- **実行フロー**: `Scheduler::shutdown` は Driver 停止→catch-up drain の直後に `TaskRunOnCloseQueue` を実行し、結果を `TaskRunOnCloseResult { succeeded, failed }` に集計する。失敗時も後続タスクは継続し、`SchedulerWarning::TaskRunFailed { task_id, error }` を EventStream/Diagnostics へ送出する。
- **観測**: Diagnostics に `task_run_on_close_total`, `task_run_on_close_failures`, `task_run_on_close_duration` を追加し、CI/運用で shutdown パスを検証できるようにする。
- **登録責務**: `ActorSystemBuilder` は以下の順に `register_on_close` を呼び出す。
  1. `DelayProvider::new`（SystemCritical）: pending delay futures を wake / cancel するタスク。
  2. `Remoting::init`（SystemCritical）: heartbeat/transport タイマーの停止と quarantined authority の flush。
  3. `ActorTimers`（Runtime）: `Context::schedule_*` 経由で生成された per-actor タイマーのキャンセル。
  4. `UserHookRegistry`（User）: ユーザが `ActorSystem::on_shutdown` で登録した cleanup。
  Builder は各 `TaskRunHandle` を `ActorSystemHandles` に保存し、`ActorSystem::terminate` で順序付きに `Scheduler::shutdown` へ渡す。

##### TaskRunOnClose Registration Sequence
```mermaid
sequenceDiagram
  participant Builder
  participant Scheduler
  participant DelayProvider
  participant Remoting
  participant ActorTimers
  Builder->>Scheduler: register_on_close(DelayProviderCleanup, SystemCritical)
  Scheduler-->>Builder: TaskRunHandle(dp)
  Builder->>DelayProvider: store_handle(dp)
  Builder->>Scheduler: register_on_close(RemotingCleanup, SystemCritical)
  Builder-->>Remoting: handle(rem)
  Builder->>Scheduler: register_on_close(ActorTimersCleanup, Runtime)
  Builder-->>ActorTimers: handle(timers)
 Builder->>Scheduler: register_on_close(UserHookChain, User)
  Builder-->>Builder: handles(User)
  Builder->>ActorSystemHandles: persist {dp, rem, timers, user}
```
このシーケンスにより、`ActorSystem::terminate` が呼ばれると Builder が保持している handles を `Scheduler::shutdown(TaskRunContext { handles })` へ渡し、Requirement 3 AC6 の「close 中に TaskRunOnClose をすべて実行する」ことを検証可能にする。

`TimerWheel::drain()` で収集した未処理タスクは Scheduler 内部で `run_in_shutdown` ハーネスを通じて順次実行または `CompletionToken::mark_cancelled()` を呼んでキャンセルする。SystemMailbox や Dispatcher へは新規 enqueue を行わず、shutdown フェーズの決定性を保ったまま R3 AC3/R3 AC7 を満たす。


#### Pekko API シグネチャ写像
| Pekko Scheduler API (Scaladoc) | Rust public API シグネチャ案 | Actor 配送セマンティクス | 備考 |
| --- | --- | --- | --- |
| `scheduleOnce(delay, receiver, message, dispatcher, sender)` | `pub fn schedule_once<M: SchedulerMessage>(delay: Duration, receiver: ActorRef<M>, message: M, dispatcher: DispatcherId, sender: Option<ActorRef<AnyMessage>>) -> Result<SchedulerHandle, SchedulerError>` | 満期時に SystemMailbox が既存の `DispatcherEnvelope` 形式のまま `message: M` を投函し、並列して `SchedulerBatchContext` へ `ExecutionBatch::once()` をプッシュする。Actor 側は `ctx.scheduler_batch()` で batch を参照でき、メッセージ型は従来通り `M` のまま維持される。 | Backpressure 時は `Err(SchedulerError::Backpressured)`、quota 超過は `Err(SchedulerError::QuotaExceeded)`、負またはゼロ遅延は `Err(SchedulerError::InvalidDelay)` を返す。 |
| `scheduleAtFixedRate(initialDelay, interval, receiver, message, dispatcher, sender)` | `pub fn schedule_at_fixed_rate<M>(initial_delay: Duration, interval: Duration, receiver: ActorRef<M>, message: M, dispatcher: DispatcherId, sender: Option<ActorRef<AnyMessage>>) -> Result<SchedulerHandle, SchedulerError>` | `FixedRateContext` が `missed_runs` を保持し、`SchedulerRunner` が発火ごとに `ExecutionBatch { runs, missed_runs, mode: FixedRate }` を `SchedulerBatchContext` へセットする。Actor/Typed API は `ctx.scheduler_batch().missed_runs()` で累積実行数を取得でき、メッセージ型は変化しない。 | `Backpressure` 状態では低優先度ジョブを拒否し `Err(SchedulerError::Backpressured)`、`Rejecting` 状態では全ジョブを fail-fast する。interval が 0 の場合は `Err(SchedulerError::InvalidDelay)`。 |
| `scheduleWithFixedDelay(initialDelay, delay, receiver, message, dispatcher, sender)` | `pub fn schedule_with_fixed_delay<M>(initial_delay: Duration, delay: Duration, receiver: ActorRef<M>, message: M, dispatcher: DispatcherId, sender: Option<ActorRef<AnyMessage>>) -> Result<(SchedulerHandle, CompletionToken), SchedulerError>` | FixedDelay は **完了通知が届くまで次回スケジュールを行わない**。SystemMailbox は `CompletionToken` を batch コンテキストへ保存し、Actor は `ctx.complete_fixed_delay(token)` を呼んで `Scheduler::ack_complete(token, finished_at)` をトリガする。Scheduler は `finished_at + delay` で次回 deadline を再計算し、`FixedDelayContext` に記録する。 | Backpressure 時に自動キャンセルされ `Err(SchedulerError::Backpressured)` を返す。`delay <= 0` なら `SchedulerError::InvalidDelay`。 |
| `scheduleOnce(delay, runnable, executor)` 等 Runnable 版 | `pub fn schedule_once_fn<F>(delay: Duration, dispatcher: DispatcherId, f: F) -> Result<SchedulerHandle, SchedulerError> where F: BatchAwareRunnable` | Runnable 版は `BatchAwareRunnable::run(&self, batch: ExecutionBatch)` を必須とし、SystemMailbox は Runnable 実行前に `SchedulerBatchContext` へ batch を push する。FixedDelay モードでは Runnable にも `CompletionToken` が引数で渡され、`complete_fixed_delay` を明示呼び出しする。 | Runnable/Future でも Backpressure/Quota/InvalidDelay を `Result` で受け取り、呼び出し側が再試行ポリシーを決められる。 |

##### SchedulerExecutionContext と公開 API 互換性
- **SchedulerExecutionContext**: Pekko の `ExecutionContext` に対応する `pub trait SchedulerExecutionContext` を導入し、`fn dispatcher(&self) -> DispatcherId` と `fn sender(&self) -> Option<ActorRefAny>` を提供する。ActorSystem は `system.scheduler().default_context()` を返し、ActorContext は `impl SchedulerExecutionContext for ActorContext` によって dispatcher/sender を透過的に解決する。
- **API 形状**: 公開 API を `pub fn schedule_once<M>(delay: Duration, target: impl Into<SchedulerTarget<M>>, ctx: &impl SchedulerExecutionContext)` のように `ctx` 引数を受け取る形へ改訂し、非アクターコードは `let ctx = system.scheduler().execution_context("remote-heartbeat")` のように命名付き Dispatcher を取得できる。Runnable 版は `schedule_once_fn(delay).with_context(&ctx).run(f)` という builder 形式を提供し、Pekko のカリー化シグネチャと対応させる。
- **取得経路**: Actor 以外の利用者（Remoting 初期化、DelayProvider 以外の subsystem）は `ActorSystem::scheduler_execution_context(name)` で Dispatcher レジストリからハンドルを受け取り、名前が `None` の場合は default dispatcher を返す。
- **互換性**: 旧 API（DispatcherId 直指定）は `#[deprecated]` ラッパーとして残し、コンパイラメッセージで `SchedulerExecutionContext` への移行を促す。Requirement R3 AC5 へのトレースを Requirements Traceability 表へ追加し、Pekko の移植コードが dispatcher/sender を意識せずに移行できることを保証する。

`DispatcherId` は既存 Dispatcher レジストリが管理され、`SchedulerExecutionContext` を通じて暗黙に供給される。Actor 側のユーティリティ（例: `Context::schedule_once`) では `impl SchedulerExecutionContext for ActorContext` を利用し、Pekko 互換 API の表面仕様を明示的に固定する。

`SchedulerBatchContext` は std では thread-local、no_std では `RuntimeToolbox::task_local_slot()`（critical-section + per-task スロット）上に構築した `TaskLocalBatch` で実装する。SystemMailbox は message/Runnable をデキューする直前に `SchedulerBatchContext::push(ExecutionBatch, CompletionToken?)` を呼び、Drop 時に pop するため、割り込み／マルチタスク下でもデータ競合が発生しない。Actor は `Context::scheduler_batch()`、Runnable は `BatchAwareRunnable::run(&self, batch: ExecutionBatch)`、DelayFuture は `DelayFuture::current_batch()` でメタデータを参照でき、公開シグネチャを変えず Requirement R2 AC2/AC4/AC5 を満たす。

##### ExecutionBatch / CompletionToken 公開契約
- **メッセージ配送**: メッセージ型 `M` はそのまま mailbox に流れ、`SchedulerMessageExt::batch(&self)` が `ExecutionBatch` を task-local から参照する。`ExecutionBatch` は `Copy` で、アプリケーションがログやメトリクスへ転送できる。
- **Runnable/Task**: Runnable 版 API は `pub trait BatchAwareRunnable { fn run(&self, batch: ExecutionBatch, completion: Option<CompletionToken>); }` を新設し、builder が `ExecutionBatch` と `CompletionToken` を渡す。従来の `FnOnce()` 互換 API は内部でこの trait を実装しており、ユーザが追加作業なしで `missed_runs` や completion を扱える。
- **CompletionToken**: FixedDelay/TaskRunOnClose など完了時刻が必要なタスクには `CompletionToken` を付与し、Actor/Runnable は `ctx.complete_fixed_delay(token)` もしくは `completion.complete(now)` を呼ぶことで `Scheduler::ack_complete(token, finished_at)` をトリガする。SystemMailbox/Dispatcher は token を意識せず、task-local を介して受け渡すだけで済む。
- **Task local の安全網**: `SchedulerBatchContext::current()` は `Option<BatchContext>` を返し、task local 機構を提供できない no_std 環境では Toolbox 実装が `TaskLocalSlot` を `critical-section` 上に確保する。どうしても確保できない極小環境向けには API から明示的に `ExecutionBatch` を受け取る builder を用意する。
- **ドキュメント更新**: Requirements Traceability の R2 行へ `ExecutionBatch + CompletionToken 公開 API` を追記し、ユーザ向けガイド（docs/guides/scheduler.md）にも `ctx.complete_fixed_delay(token)` の使用例を追加する。

##### FixedDelay Completion Sequence
```mermaid
sequenceDiagram
  participant Scheduler
  participant TimerWheel
  participant SystemMailbox
  participant Actor
  Scheduler->>TimerWheel: insert FixedDelayEntry(handle_id, delay)
  TimerWheel-->>Scheduler: deadline reached (handle_id)
  Scheduler->>SystemMailbox: enqueue message + BatchContext{batch, completion_token}
  SystemMailbox->>Actor: deliver message (ctx.scheduler_batch() exposes token)
  Actor->>Scheduler: ctx.complete_fixed_delay(token)
  Scheduler->>Scheduler: compute next_deadline = finished_at + delay
  Scheduler->>TimerWheel: reinsert FixedDelayEntry(handle_id, next_deadline)
```

`CompletionToken` を task-local 経由で受け渡すことで、Dispatcher/SystemMailbox に新たなフックを追加せず、Actor が明示的に完了通知を呼ぶだけで Requirement R2 AC4 を満たせる。

#### SystemMailbox ブリッジ詳細
1. `Scheduler::schedule_*` は `SchedulerCommand::EnqueueDelayed { receiver, dispatcher, sender, payload, handle_id }` を生成し、TimerWheel にエントリを登録する。
2. `run_tick` で期限到来したコマンドを取り出し、`SystemMailboxBridge` へ転送。Bridge は既存の `DispatcherEnvelope { dispatcher_id, receiver_pid, sender, message }` を構築し、同時に `BatchContext { execution_batch, completion_token, handle_id }` を添えて `SystemMailbox::enqueue_system` を呼ぶ。
3. `SystemMailbox` は既存 `SystemMessage` 優先ルールを維持しつつ、`UserMessagePriority::Delayed` で処理する。enqueue 直前に `CancellableRegistry::is_cancelled(handle_id)` を参照し、キャンセル済みであれば破棄して `SchedulerMetrics::dropped_total` を更新する。enqueue が成功した場合のみ `SchedulerBatchContext::push(batch, completion)` が呼ばれ、その後の Actor 実行中に `ctx.scheduler_batch()` から batch/CompletionToken を参照できる。
4. `DelayProvider` や内部ユーティリティは `SchedulerFacade` を介して上記 API をラップし、ActorRef を持たないケース（Future waker 等）でも SystemMailbox を経由した deterministic な配送を確保する。このとき `SchedulerFacade` は `ExecutionBatch` を `DelayFutureWaker` へ引き渡し、Runnable/Future 系タスクでもミス Tick 情報や CompletionToken を失わない。

このブリッジ手順により、Pekko/Proto.Actor が期待する「Scheduler は直接アクターを実行せず、mailbox 経由で保証された順序を保つ」という要件 (R3 AC4) を Rust 実装にも適用できる。

#### CancellableRegistry と競合セマンティクス
- **同期原語**: `CancellableState`（`Pending`, `Scheduled`, `Executing`, `Cancelled`）を `ToolboxAtomicU8` で保持し、ロックレスに状態遷移を制御する。`SchedulerHandle` は `state.compare_exchange` により自身の状態を更新し、`CancellableRegistry` も同じ原語を参照する。
- **状態機械**:

| 現状態 | イベント | 新状態 | 備考 |
| --- | --- | --- | --- |
| Pending | `schedule_*` 成功 | Scheduled | TimerWheel へ登録済み。`handle_id` を割り当て。 |
| Scheduled | `cancel()` (成功) | Cancelled | `TimerWheel::cancel(handle_id)` へ伝播。戻り値 true を返す。 |
| Scheduled | `run_tick` で取得 | Executing | `run_tick` が `compare_exchange(Scheduled→Executing)` に成功した場合のみハンドラ実行。 |
| Executing | ハンドラ完了 | Cancelled | 実行終了後に `state.store(Cancelled)`。`cancel()` は false を返し idempotent。 |
| Cancelled | 以降の `cancel()` | Cancelled | 常に false を返却。 |

- **競合解消**:
  1. `cancel()` は `state.compare_exchange(Scheduled→Cancelled)` のみを成功条件とし、すでに `Executing/Cancelled` の場合は false を返して Requirement R4 AC2-AC3 を満たす。
  2. `run_tick` は `Scheduled` 状態のエントリだけを取り出し、`Executing` へ遷移できなかった場合（= cancel 済み）にはハンドラを呼ばずに破棄する。
  3. `SystemMailboxBridge` が生成する `BatchContext` には `handle_id` を含め、enqueue 直前に `CancellableRegistry::is_cancelled(handle_id)` を参照する。キャンセル済みなら BatchContext を破棄し、ユーザ視点で「is_cancelled==true なのにメッセージが届く」ことを防ぐ。
  4. `cancel()` が `Executing` へ遷移済みのエントリに競合した場合、戻り値は false だがハンドラは既に 1 回のみ実行中であり、実行完了時に `state.store(Cancelled)`。SystemMailbox 側も `handle_id` を見て enqueue 済みメッセージを破棄するため二重配送にならない。
- **is_cancelled レポート**: `SchedulerHandle::is_cancelled()` は `state.load(Ordering::Acquire) == Cancelled` で判定し、`run_tick` 側の `store` と `cancel()` 側の `compare_exchange` を `Release` で行うことで可視性を保証。R4 AC4 を満たす。
- **多重登録/Quota**: `CancellableRegistry` は `system_quota` チェック後に `Pending→Scheduled` への遷移を行い、Quota 超過時には state を `Cancelled` へ即座に更新して `SchedulerError::QuotaExceeded` を返す。これにより、キャンセル済みハンドルが wheel に残らず、バックプレッシャ計測も正確に行える。

このセマンティクスにより、キャンセルと発火が競合してもハンドラが二重に実行されず、`cancel()` の戻り値規約（初回 true/以降 false）と `is_cancelled` の一貫性を保証する。

#### 診断・ダンプ API（Requirement 5）
- **ManualClock Harness (R5 AC1)**: `ManualClock` と `ManualTimer` は `SchedulerRunner` と同じ `run_tick` を直接呼び出す `advance(duration)` 実装を持つ。`SchedulerDiagnostics::with_manual_clock(clock: ManualClockHandle)` を呼ぶと `RunnerMode::Manual` に切り替わり、テストコードは `manual_clock.advance(n)` → `scheduler.diagnostics().last_fire_batch()` の順で determinism を検証できる。
- **Deterministic Log (R5 AC2)**: `SchedulerDiagnostics` に `enable_deterministic_log(buffer: &'static mut RingBuffer<SchedulerFireRecord>)` API を設け、各 `schedule/fire/cancel` で `SchedulerFireRecord { timer_id, scheduled_at, deadline, fired_at, executor }` を記録。ログは `SchedulerReplayTrace` としてシリアライズ可能で、リプレイテストや fuzz の seed 入力として利用する。
- **診断ストリーム (R5 AC3)**: `DiagnosticsChannel` を `EventStream` のサブチャンネルとして追加し、`SchedulerDiagnostics::subscribe(kind)` で `ScheduleEvent`, `FireEvent`, `CancelEvent`, `DriftWarning` などを購読可能にする。各イベントは `SchedulerDiagnosticEvent` enum にまとめ、no_std でも利用できるよう `heapless::spsc::Queue` をバックエンドへ採用。
- **Property/Fuzz API (R5 AC4)**: `SchedulerProperties` モジュールに `fn assert_monotonic(&self)` や `fn inject_random_cancel(seed)` を提供し、プロパティテストや fuzz ハーネスから `SchedulerHarness` 経由でスケジューラ内部状態へアクセスできるようにする。`ManualClock` + `DeterministicLog` を組み合わせ、tick 単調性や固定レート補償の不変条件を検証する。
- **Dump 要求 (R5 AC5)**: `SchedulerDiagnostics::dump(fmt: DumpFormat)` が `SchedulerDump` 構造体を生成し、`DumpFormat::Text` では `wheel_offset`, `active_slots`, `overflow_size`, `periodic_jobs[{job_id, mode, backlog, last_fire}]`, `pending_commands` を表形式で整形。`DumpRequest` は `ActorSystemDiagnostics` 経由で CLI/テレメトリから呼び出せるよう、`SchedulerDumpRequest` メッセージを SystemMailbox へ送って `SchedulerDumpReply` を返す。no_std では `fmt::Write` ベース、std では `io::Write` にも対応させ、埋め込みとホストの双方でダンプを取得できる。

これらの診断 API を通じて、Requirement 5 の手動 tick、決定論ログ、ストリーム通知、プロパティ検証、ダンプ出力の全要件を Scheduler 単体で満たす。

### actor_core/integration
- **ActorSystemBuilder**: `with_scheduler_config`、`build()` 内で RuntimeToolbox から clock/timer を取得し、Scheduler を初期化。
- **Builder Lifecycle Hooks**: `ActorSystemBuilder::build()` は Scheduler を初期化した直後に (a) `register_on_close` で system-critical cleanup（DelayProvider, Remoting heartbeat 等）を登録し、(b) `SystemActorBootstrap` に `TaskRunHandle` を引き渡す。Builder は shutdown パスでも `task_run_handles` を保持し、`ActorSystem::terminate()` から `Scheduler::shutdown(TaskRunContext)` へ優先度情報を渡して `TaskRunOnClose` が deterministic に実行されるようにする。
- **SystemMailboxBridge**: Scheduler からの発火を system mailbox に enqueue。
- **DelayProvider registration**: ActorSystem 構築時に SchedulerBackedDelayProvider を各コンポーネントへ注入。
- **SchedulerRunnerShell**: actor-core 内で `TickSource` から `TickEvent` を pull して `run_tick` を呼ぶ薄いシェル。`TickSourceStatus::{Ok, Backpressured, Suspended}` を監視し、`Backpressured` を受信した瞬間に (1) `pending_ticks` 回ぶんの catch-up ループを同期実行し、(2) `SchedulerBackpressureLink::raise(Driver)` を呼んで accepting_state を Backpressure へ遷移させる。`pending_ticks` が `0.8 * max_pending` 未満に戻ったら `SchedulerBackpressureLink::clear(Driver)` を通じて通常状態へ復帰させる。Shell 自体は tokio/SysTick を知らず、Toolbox 側が提供する TickSource 実装を差し替えるだけで std/no_std を横断できる。

### actor_std implementations
- **StdInstantClock**: `MonotonicClock` for std。`Instant::now()` をナノ秒へ変換。
- **StdTimerWheelImpl**: `TimerWheelFamily` for std。`tokio::time::sleep_until` や wake-up 処理は actor-std の RuntimeToolbox 実装に閉じ込め、actor-core の `SchedulerRunner` には抽象化された `TickSource` のみを渡す。これにより `cfg(feature = "std")` を actor-core に導入せず、ステアリングの no_std 方針を維持する。

## Capacity & Backpressure
- **Backpressure Link**: ActorSystem 側の `BackpressureGauge`（mailbox 飽和率、Dispatcher の拒否状態、RuntimeToolbox が公開する `backpressure_token()`）と Scheduler を `SchedulerBackpressureLink` で接続する。Gauge が `State::Engaged` になった瞬間に `SchedulerConfig` が `accepting_state = Backpressure` へ遷移し、以降は低優先度タイマーを拒否 or 低頻度でのみ受理する。状態遷移は EventStream `SchedulerWarning::BackpressureState { engaged: bool, reason }` で通知し、DelayProvider や上位コンポーネントが統一的に観測できるようにする。
- **Driver 連携**: `DriverStatus::Backpressured { pending_ticks, overflow }` を受け取った場合、`SchedulerBackpressureLink::raise(Driver)` を呼び出し `BackpressureGauge` に driver 由来の理由をセットする。`pending_ticks` が `max_pending_ticks` の 80% を下回ったら `SchedulerBackpressureLink::clear(Driver)` を呼び、通常状態へ戻す。これにより catch-up backlog の逼迫がスケジューラ全体の受理ポリシーへ反映される。

###### Backpressure State Machine
```mermaid
stateDiagram-v2
    [*] --> Normal
    Normal --> DriverBackpressured: DriverStatus::Backpressured
    Normal --> GaugeBackpressured: BackpressureGauge::engaged
    DriverBackpressured --> Backpressure: SchedulerBackpressureLink::raise(Driver)
    GaugeBackpressured --> Backpressure: SchedulerBackpressureLink::raise(Gauge)
    Backpressure --> Normal: pending_ticks < 0.8 * max_pending && gauge.clear()
    Backpressure --> Rejecting: adaptive_quota exhausted
    Rejecting --> Normal: cooldown elapsed
```
- `Backpressure` 状態では `SchedulerConfig::accepting_state = Backpressure` とし、低優先度タイマーや `schedule_with_fixed_delay` を拒否/遅延させる。
- `Rejecting` は adaptive quota が 0 になった状態を示し、すべての新規タイマーが `SchedulerError::Backpressured` で fail-fast する。cooldown（デフォルト 5 * resolution）経過で `Normal` へ復帰する。
- **SystemTimerQuota**: `SchedulerConfig` に `system_quota`（同時稼働タイマーと周期ジョブの合計上限）を持たせ、`active_total >= system_quota` の場合は `SchedulerError::QuotaExceeded` を返す。`BackpressureLink` が `Engaged` の間は `adaptive_quota = system_quota / 2`（設定可能）を適用し、解除時に元へ戻す。拒否時には `SchedulerWarning::QuotaReached { limit, adaptive }` を EventStream へ発行し、上位で制御判断ができるようにする。
- **FixedRatePolicy**: `SchedulerPolicyRegistry` が `FixedRatePolicy`（`backlog_limit`, `burst_threshold`, `priority_class`）を保持し、`schedule_at_fixed_rate/with_fixed_delay` 呼び出し時に該当ポリシーを注入する。これにより公開 API に追加引数を設けずに Requirement 2 AC6（保留数 k 管理）と Requirement 4 AC6（優先度別ドロップ）を満たす。
- **TimerPriority**: `TimerEntry` に `priority: PriorityClass (High|Normal|Low)` を持たせ、優先度は `SchedulerPolicyRegistry` が dispatcher/guardian などの `SchedulerAffinity` から解決する。デフォルトで ActorSystem 内部用途=High、ユーザジョブ=Normal/Low を割り当て、Pekko 互換 API には追加引数を要求しない。
- **Wheel Saturation Policy**: `TimerWheelFamily` がスロット/overflow いずれかの容量しきい値に達した場合、`PriorityDropQueue` が `Low -> Normal -> High` の順にエントリを選び、キャンセル処理を伴う `SchedulerWarning::DroppedLowPriority { job_id, priority }` を EventStream へ通知する。ドロップ後は `active_total` から差し引き、`system_quota` の範囲に戻るまで処理を繰り返す。
- **Metrics**: `SchedulerMetrics` に `active_total`, `periodic_total`, `dropped_total`, `quota_limit` を追加し、バックプレッシャ状態の監視と要件 R4 AC5-AC7 の可視化を保証する。
- **Overflow Reinsertion Strategy**: `TimerWheelConfig` に `overflow_heap_capacity`（デフォルト `system_quota` と同数）と `overflow_reinsertion_threshold_pct`（デフォルト 75%）を設け、overflow heap の使用率が閾値を超えた場合に再挿入プロセスを起動する。`run_tick` ごとに `reinsertion_batch_limit`（デフォルト 100 エントリ）まで遠未来タイマーを wheel へ戻し、1 tick あたりの再挿入時間を限定する。再挿入で wheel 範囲へ収まり切らない場合は次 tick 以降に持ち越し、同時に `SchedulerWarning::OverflowNearCapacity` を EventStream へ送出して監視側へ通知する。ドロップ判定は再挿入後も `overflow_heap_capacity` を超過した時点で `Wheel Saturation Policy` にフォールバックする。
    - `overflow_heap_capacity` と `overflow_reinsertion_threshold_pct` は `heap_len >= capacity * threshold / 100` を条件に再挿入をトリガする。例: capacity=10_000, threshold=75 なら heap サイズ 7_500 超で起動。
    - 再挿入アルゴリズム擬似コード:
      ```text
      fn reinsertion_pass(now_tick):
          moved = 0
          while moved < reinsertion_batch_limit && overflow_heap.peek_deadline() <= wheel_max_deadline(now_tick):
              entry = overflow_heap.pop_min()      // O(log n)
              slot = calc_slot(entry.deadline)
              if slot.is_full():
                  break    // wheel saturation policy へ委譲
              slot.push(entry)
              moved += 1
          if overflow_heap.len() > overflow_heap_capacity:
              drop_low_priority_entries()
      ```
    - Worst-case コストは `O(log n) * reinsertion_batch_limit`（デフォルト 100）で、`resolution=10ms` でも 5% ドリフト内に収まるよう Performance セクションで基準値を提示する。
    - `Wheel Saturation Policy` との優先順位: (1) overflow 再挿入 → (2) wheel slot 飽和検知 → (3) priority drop の順に評価する。

## Error Handling
- **InvalidDelay**: `SchedulerError::InvalidDelay` として `delay <= 0` や `delay / tick_nanos > IntMax` を返却。呼び出し元は Result ハンドリング。
- **CapacityExceeded**: TimerWheel スロットが飽和した場合 `SchedulerError::CapacityExceeded` を返し、EventStream に警告。
- **DriverStalled**: `pending_ticks > max_pending_ticks` が解消しないまま `TickSourceStatus::Stalled` が返ってきた場合、Scheduler は `accepting_state = Rejecting` へ遷移し新規要求へ `SchedulerError::Backpressured(Driver)` を返す。同時に EventStream へ `SchedulerWarning::DriverCatchUpExceeded` を出力し、診断ストリームで backlog 状態を共有する。既存のタイマーは `FixedRatePolicy` / `PriorityDropQueue` に従う通常のドロップポリシーを用いるため、driver 由来で勝手に破棄されることはない。
- **ShutdownInProgress**: shutdown 後の schedule 呼び出しは `SchedulerError::Shutdown`。
- **TaskRunOnClose Failure**: handler 内エラーは捕捉せず propagate、ただし EventStream で通知し subsequent handler が継続できるようにする。
- **Handler Panic Strategy**: `TimerEntry` の `payload` は `SchedulerTask` trait（`fn run(&self, batch: ExecutionBatch) -> Result<(), HandlerError>`）を実装し、`run_tick` は `Ok(Err(e))` を検知した際に `SchedulerWarning::HandlerFailed { job_id, error: e, batch }` を EventStream へ送る。panic に対しては catch_unwind 等で介入せず（no_std 方針で unwind 非対応）、タスク側は panic-free であること、もしくは panic 発生時にはランタイムが即 abort することを前提にする。`CancellableRegistry` は失敗したジョブを `is_cancelled = true` に更新し二重実行を防止する。Actor メッセージ処理は `ActorCellGeneric::invoke_user_message`（`modules/actor-core/src/actor_prim/actor_cell.rs`）で従来どおり直接 `Actor::receive` を呼び出し、panic を捕捉しないポリシーを維持する。
- **Overflow/Wheel Saturation 順序**: `run_tick` 内では (1) overflow 再挿入、(2) wheel slot 飽和検知、(3) priority drop の順に判定し、前段で条件を満たした場合は即座に次フローへ委譲する。これにより Requirement 4 AC6 の優先順位をドキュメントで固定する。
- **Enqueue 後キャンセル**: `BatchContext` に `handle_id` を含め、SystemMailbox が配送直前に `CancellableRegistry::is_cancelled(handle_id)` をチェックする。キャンセル済みであれば `SchedulerMetrics::dropped_total` を増やしつつ破棄し、Requirement 4 AC2-AC4 の厳密な保証を守る。

## Testing Strategy
- **Unit**: TimerWheel tick/overflow、SchedulerHandle cancel、ManualClock advance、Diagnostics dump。
- **Integration**: ActorSystem + Scheduler で schedule_once/fixed_rate/fixed_delay、shutdown TaskRunOnClose、DelayProvider 経由の timeout。
- **Property**: ドリフト検証（`advance` vs expected deadline）、固定レート補償（GC バーストシミュレーション）。
- **Performance**: no_std wheel ベンチ（1k/10k timers）、overflow heap 再挿入コスト、std 実装での burst ハンドリング。
- **Performance**: no_std wheel ベンチ（1k/10k timers）、overflow heap 再挿入コスト（`O(log n) * reinsertion_batch_limit` が tick あたり 100 pop/push 以内に収まることを確認）、std 実装での burst ハンドリング。
- **Backpressure/CI Plan**:
  1. `scripts/ci-check.sh backpressure` を追加し、`catch_up_window_ms` を 10/50/100ms に切り替えた 3 パターンで `max_pending_ticks` が計算通りになるか property テストする。
  2. Driver モックを用い、`pending_ticks` を `0.8 * max_pending` 超まで増やして `DriverStatus::Backpressured` を発火させ、`SchedulerBackpressureLink` が `accepting_state = Backpressure` へ遷移することを確認。
  3. Adaptive quota が 0 になるまで `schedule_once` を呼び、`SchedulerError::Backpressured` が返ること、cooldown（5 * resolution）経過後に受理が再開することを integration テストで検証。
  4. CI では std/no_std 双方で上記テストを実行し、`tick_backlog_peak`, `tick_overflow_events`, `task_run_on_close_total` を metrics snapshot として収集する。

## Performance & Scalability
- **Targets**: 1 tick = 10ms デフォルト、最大遅延 = 2^20 tick、1 アクターシステムあたり同時タイマー 10k。
- **Scaling**: wheel サイズは設定可能、overflow は `BinaryHeap` + `SmallVec` で保持し `wheel_range_threshold = ticks_per_slot * slots` に近づいた時点で再挿入。std 版は tokio task で `SchedulerRunner` が tick を駆動。
- **Metrics**: active timers, periodic jobs, dropped timers を `SchedulerMetrics` にエクスポート。
- **Catch-up 設定指針**: `max_pending_ticks = ceil((worst_case_pause_ms * 1_000_000) / resolution_ns)` を基本式とし、Cortex-M (SysTick 1kHz, resolution 1ms) では `max_pending_ticks <= 8`, Linux host (tokio, resolution 10ms) では `<= 32` を推奨する。`MAX_PENDING = 64` を超える設定は `SchedulerConfig::validate()` が拒否し、`SchedulerWarning::DriverCatchUpExceeded` が一度でも発火した場合は `catch_up_window_ms` を 25% 減らして再計算する運用ルールを docs/guides/scheduler.md へ追記する。

## Migration Strategy
```mermaid
flowchart TD
  phase0[Phase0 現状 DelayProvider 維持] --> phase1[Phase1 Scheduler 稼働 + DelayProvider 内部置換]
  phase1 --> phase2[Phase2 Mailbox/Queue で Scheduler 直接利用]
  phase2 --> phase3[Phase3 DelayProvider 非推奨化]
```
- **Rollback**: Phase1 では旧 DelayProvider 実装を feature flag で残し、問題時に切り替え可能。
- **Validation**: 各 phase で `scripts/ci-check.sh all` と専用 Scheduler テストスイートを実行。
- **Feature Flag 詳細**: Phase1 では Cargo feature `scheduler-new`（デフォルト有効）で新 Scheduler を利用し、`--no-default-features --features scheduler-legacy` で旧 DelayProvider に切り替え可能とする。ランタイム本体では cfg を導入せず DelayProvider 実装層のみで切り替える。
- **置換対象リストと優先度**: Phase1 で `DelayFuture`, `Mailbox::poll_timeout`, `RemoteAuthority` タイムアウトなど `rg "DelayProvider"` で洗い出した 12 箇所を優先置換。Phase2 では Mailbox/Queue が直接 Scheduler API を呼ぶよう改修し、DelayProvider は facade のみを残す。
- **Rollback ポリシー**: プロジェクト方針「破壊的変更を恐れずに最適化」を尊重し、rollback は Phase1 の検証期間（CI + nightly ベンチマーク）に限定。Phase2 以降は `scheduler-new` を唯一のパスとし、不具合時は spec/design の追補で対処する。
### 周期ジョブのバックログ処理
```mermaid
sequenceDiagram
  participant Caller
  participant Scheduler
  participant Runner
  participant TimerWheel
  participant EventStream
  Caller->>Scheduler: schedule_fixed_rate(p, handler, mode)
  Scheduler->>TimerWheel: insert(FixedRateEntry)
  Runner->>Scheduler: run_tick(now)
  Scheduler->>TimerWheel: advance(now)
  TimerWheel-->>Scheduler: expired(FixedRateBatch { missed })
  Scheduler->>Scheduler: FixedRateContext::fold_missed(missed)
  alt backlog <= policy.backlog_limit
    Scheduler->>SystemMailbox: enqueue(handler, runs = missed+1)
  else backlog > policy.backlog_limit
    Scheduler->>Scheduler: cancel_entry()
    Scheduler->>EventStream: warn(BacklogExceeded)
  end
  Scheduler->>EventStream: warn(BurstFire) note over Scheduler,EventStream: mode == FixedRate && missed > burst_threshold
```

`FixedRateContext` は各周期ジョブに紐づく状態であり、`last_fire`, `missed_runs`, `mode`（FixedRate/FixedDelay）と `backlog_limit` を保持する。`run_tick` は `TimerWheel` から受け取った `FixedRateBatch` を `missed_runs` に折り畳み、`handler` へ `ExecutionBatch { runs: u32, mode }` を渡す。`backlog_limit` を超えた場合はジョブをキャンセルし `SchedulerWarning::BacklogExceeded { job_id }` を EventStream へ出力する。GC などで `missed_runs` が `burst_threshold`（デフォルト `resolution * 10` 相当）を超えたときは `SchedulerWarning::BurstFire` を送信し、上位で観測できるようにする。これらの `backlog_limit`/`burst_threshold` は `SchedulerPolicyRegistry` の `FixedRatePolicy` から供給され、公開 API から追加引数を受けることなくポリシーを差し替えられる。
- **Backpressure/Rejecting 応答**: `accepting_state = Backpressure` の間、`schedule_*` は優先度 `High` の内部タイマーのみ受理し、それ以外は `Err(SchedulerError::Backpressured)` を返す。`accepting_state = Rejecting`（adaptive quota=0）ではすべての `schedule_*` が fail-fast し、呼び出し側は `SchedulerError::Backpressured` を受けて再スケジュールを検討する。`schedule_with_fixed_delay` 等の低優先度 API は Backpressure 時に自動キャンセルされ、呼び出し元へ `SchedulerWarning::CancelledByBackpressure` を送る。cooldown が終わり Normal へ戻った時点で再度登録可能。
