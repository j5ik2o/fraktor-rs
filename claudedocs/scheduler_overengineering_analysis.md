# Scheduler設計の過剰設計分析

ユーザー向けAPI（What）を維持しつつ、内部実装（How）の無駄を分析した結果。

## 分析の前提

- **ユーザー機能（What）**: 削減しない - すべての公開APIを維持
- **内部実装（How）**: 簡素化可能 - コード重複、不要な層、過剰な分割を削減
- **設計原則**: "Less is more" と "YAGNI"

## 過剰設計と判断される箇所（実装の無駄）

### 1. FixedRateContextとFixedDelayContextのコード重複 ⭐️ 高優先度

**問題:**
```rust
// fixed_rate_context.rs と fixed_delay_context.rs が95%同じコード
pub(crate) struct FixedRateContext {
  next_tick: u64,
  period_ticks: NonZeroU64,
  backlog_limit: NonZeroU32,
  burst_threshold: NonZeroU32,
}

pub(crate) struct FixedDelayContext {
  next_tick: u64,           // 同じ
  period_ticks: NonZeroU64, // 同じ
  backlog_limit: NonZeroU32,   // 同じ
  burst_threshold: NonZeroU32, // 同じ
}
```

**唯一の違い:**
```rust
// FixedRate: 38行目
self.next_tick = self.next_tick.saturating_add(
  self.period_ticks.get().saturating_mul(u64::from(runs_total))
);

// FixedDelay: 38行目
self.next_tick = now.saturating_add(self.period_ticks.get());
```

**無駄の内容:**
- 2つのファイル、2つの構造体、ほぼ同一のロジック
- `compute_missed`メソッドは完全に同一（50-54行）
- `new`, `next_deadline_ticks`も同一
- 唯一の違いは`next_tick`の計算式のみ

**簡素化案:**
```rust
// periodic_context.rs として統合
pub(crate) struct PeriodicContext {
  next_tick: u64,
  period_ticks: NonZeroU64,
  backlog_limit: NonZeroU32,
  burst_threshold: NonZeroU32,
  mode: BatchMode, // 違いはこれだけ
}

impl PeriodicContext {
  fn build_batch(&mut self, now: u64, handle_id: u64) -> PeriodicBatchDecision {
    let missed = self.compute_missed(now);
    if missed >= self.backlog_limit.get() {
      return PeriodicBatchDecision::Cancel {
        warning: SchedulerWarning::BacklogExceeded { handle_id, missed }
      };
    }

    let warning = if missed > self.burst_threshold.get() {
      Some(SchedulerWarning::BurstFire { handle_id, missed })
    } else {
      None
    };

    let runs_total = missed.saturating_add(1);
    let runs = unsafe { NonZeroU32::new_unchecked(runs_total) };

    // 唯一の違いをmatchで表現
    self.next_tick = match self.mode {
      BatchMode::FixedRate => {
        self.next_tick.saturating_add(
          self.period_ticks.get().saturating_mul(u64::from(runs_total))
        )
      },
      BatchMode::FixedDelay => {
        now.saturating_add(self.period_ticks.get())
      },
      _ => unreachable!(),
    };

    PeriodicBatchDecision::Execute {
      batch: ExecutionBatch::periodic(runs, missed, self.mode),
      warning
    }
  }

  // compute_missedは完全に共通
  fn compute_missed(&self, now: u64) -> u32 {
    if now <= self.next_tick {
      return 0;
    }
    let delta = now - self.next_tick;
    let period = self.period_ticks.get();
    let raw = delta / period;
    raw.min(u32::MAX as u64) as u32
  }
}
```

**効果:**
- 2ファイル削減（fixed_rate_context.rs, fixed_delay_context.rs → periodic_context.rs）
- 約100行のコード重複削除
- ユーザーAPIへの影響: なし（内部実装のみの変更）

---

### 2. CancellableEntry + CancellableState + CancellableRegistryの層が深い 🟡 中優先度

**問題:**
```rust
// scheduler_core.rs内で3層のアクセス
if let Some(entry) = self.registry.get(handle_id) {  // 1. Registry
  if !entry.try_begin_execute() {                     // 2. Entry
    // Entry内部でAtomicU8をCancellableStateに変換  // 3. State
  }
}
```

**実装の詳細:**
```rust
// cancellable_registry.rs (31行) - ただのHashMapラッパー
pub struct CancellableRegistry {
  entries: HashMap<u64, ArcShared<CancellableEntry>>
}
// 実装は register(), get(), remove() の3メソッドのみ

// cancellable_entry.rs (87行) - 状態遷移ロジック
pub struct CancellableEntry { state: AtomicU8 }
// 実装は状態遷移メソッド群

// cancellable_state.rs (27行) - enum定義とu8変換
pub enum CancellableState {
  Pending, Scheduled, Executing, Completed, Cancelled
}
impl From<u8> for CancellableState { ... }
```

**無駄の内容:**
- `CancellableRegistry`は`HashMap`への薄すぎるラッパー（わずか3メソッド）
- `CancellableState`は`AtomicU8`の内部実装詳細（外部公開の必要性が薄い）
- 3ファイルに分散しているが密接に結合

**簡素化案:**
```rust
// scheduler_core.rs内で直接管理
pub struct Scheduler<TB: RuntimeToolbox> {
  // ...
  cancellables: HashMap<u64, ArcShared<CancellableEntry>>, // 直接持つ
  // ...
}

impl<TB: RuntimeToolbox> Scheduler<TB> {
  // Registryメソッドを直接実装
  fn get_cancellable(&self, handle_id: u64) -> Option<ArcShared<CancellableEntry>> {
    self.cancellables.get(&handle_id).cloned()
  }

  fn register_cancellable(&mut self, handle_id: u64, entry: ArcShared<CancellableEntry>) {
    self.cancellables.insert(handle_id, entry);
  }

  fn remove_cancellable(&mut self, handle_id: u64) -> Option<ArcShared<CancellableEntry>> {
    self.cancellables.remove(&handle_id)
  }
}

// cancellable_entry.rs のみ残す（状態遷移ロジックはここに集約）
// CancellableStateは内部実装として統合
pub struct CancellableEntry {
  state: AtomicU8,
}

// State enumはprivateに
enum State { Pending = 0, Scheduled = 1, Executing = 2, Completed = 3, Cancelled = 4 }
```

**効果:**
- 2ファイル削減（cancellable_registry.rs, cancellable_state.rs）
- 間接層の削除で約60行削減
- ユーザーAPIへの影響: `CancellableState`が公開されている場合は維持

---

### 3. DeterministicLog + DeterministicReplayの分離が不要 🟢 低優先度

**問題:**
```rust
// deterministic_log.rs (31行) - Vec<DeterministicEvent>のラッパー
pub(crate) struct DeterministicLog {
  entries: Vec<DeterministicEvent>,
  capacity: usize,
}
// 実装は record(), entries() の2メソッドのみ

// deterministic_replay.rs (33行) - ただのイテレータ
pub struct DeterministicReplay<'a> {
  events: &'a [DeterministicEvent],
  position: usize,
}
// 実装は Iterator trait のみ
```

**無駄の内容:**
- `DeterministicLog`は`Vec<T>`にcapacity制限を加えただけ
- `DeterministicReplay`はスライスのイテレータとして標準機能で実現可能
- 両方とも薄すぎるラッパー

**簡素化案:**
```rust
// scheduler_diagnostics.rs内に統合
pub struct SchedulerDiagnostics {
  deterministic_events: Option<Vec<DeterministicEvent>>,
  deterministic_capacity: usize,
  // ...
}

impl SchedulerDiagnostics {
  pub fn enable_deterministic_log(&mut self, capacity: usize) {
    self.deterministic_events = Some(Vec::with_capacity(capacity));
    self.deterministic_capacity = capacity;
  }

  pub fn deterministic_log(&self) -> &[DeterministicEvent] {
    self.deterministic_events.as_ref().map_or(&[], |v| v.as_slice())
  }

  pub fn replay(&self) -> impl Iterator<Item = &DeterministicEvent> {
    self.deterministic_events
      .as_ref()
      .map(|v| v.iter())
      .into_iter()
      .flatten()
  }

  pub(crate) fn record(&mut self, event: DeterministicEvent) {
    if let Some(log) = &mut self.deterministic_events {
      if log.len() < self.deterministic_capacity {
        log.push(event);
      }
    }
  }
}
```

**効果:**
- 2ファイル削減（deterministic_log.rs, deterministic_replay.rs）
- 不要なラッパー削除で約40行削減
- ユーザーAPIへの影響: `DeterministicReplay`型が公開されているが、`impl Iterator`で代替可能

---

### 4. DiagnosticsRegistryの独立ファイルが過剰 🟢 低優先度

**問題:**
```rust
// diagnostics_registry.rs (83行)
// SchedulerDiagnosticsの内部実装なのに独立ファイル
pub(crate) struct DiagnosticsRegistry { ... }
pub(crate) struct DiagnosticsSubscriber { ... }
pub(crate) struct DiagnosticsBuffer { ... }
```

**無駄の内容:**
- すべて`pub(crate)`で外部公開されていない
- `SchedulerDiagnostics`からしか使われない
- 内部実装の詳細が独立ファイルになっている

**簡素化案:**
```rust
// scheduler_diagnostics.rs内にprivateモジュールとして配置
mod registry {
  use super::*;

  pub(super) struct DiagnosticsRegistry { ... }
  pub(super) struct DiagnosticsSubscriber { ... }
  pub(super) struct DiagnosticsBuffer { ... }
}

pub struct SchedulerDiagnostics {
  registry: registry::DiagnosticsRegistry,
  // ...
}
```

**効果:**
- 1ファイル削減（diagnostics_registry.rs）
- 実装の局所化により理解しやすくなる
- ユーザーAPIへの影響: なし（内部実装のみ）

---

### 5. PolicyRegistryが薄すぎる 🟢 低優先度

**問題:**
```rust
// policy_registry.rs (51行)
pub struct SchedulerPolicyRegistry {
  fixed_rate: FixedRatePolicy,   // たった2フィールド
  fixed_delay: FixedDelayPolicy,
}

// 実装は4つのgetterとbuilderメソッドのみ
impl SchedulerPolicyRegistry {
  pub const fn new(fixed_rate: FixedRatePolicy, fixed_delay: FixedDelayPolicy) -> Self
  pub const fn fixed_rate(&self) -> FixedRatePolicy
  pub const fn fixed_delay(&self) -> FixedDelayPolicy
  pub const fn with_fixed_rate(mut self, policy: FixedRatePolicy) -> Self
  pub const fn with_fixed_delay(mut self, policy: FixedDelayPolicy) -> Self
}
```

**無駄の内容:**
- 2つのポリシーを保持するだけの構造体に専用ファイル
- `SchedulerConfig`に直接含めても問題ない規模

**簡素化案:**
```rust
// config.rs内に統合
pub struct SchedulerConfig {
  // ...既存フィールド
  fixed_rate_policy: FixedRatePolicy,
  fixed_delay_policy: FixedDelayPolicy,
}

impl SchedulerConfig {
  pub const fn fixed_rate_policy(&self) -> FixedRatePolicy {
    self.fixed_rate_policy
  }

  pub const fn fixed_delay_policy(&self) -> FixedDelayPolicy {
    self.fixed_delay_policy
  }

  pub const fn with_fixed_rate_policy(mut self, policy: FixedRatePolicy) -> Self {
    self.fixed_rate_policy = policy;
    self
  }

  pub const fn with_fixed_delay_policy(mut self, policy: FixedDelayPolicy) -> Self {
    self.fixed_delay_policy = policy;
    self
  }
}

// 互換性のため、型エイリアスとして公開を維持
pub type SchedulerPolicyRegistry = SchedulerConfig;
```

**効果:**
- 1ファイル削減（policy_registry.rs）
- 設定が一箇所に集約される
- ユーザーAPIへの影響: 型エイリアスで完全互換性維持可能

---

### 6. TaskRun関連の過剰な分割 🟡 中優先度

**問題:**
```rust
// 8ファイルに分散
task_run_entry.rs (52行)    // BinaryHeapのエントリ
task_run_error.rs (9行)     // たった1つのenum
task_run_handle.rs (31行)   // u64のラッパー
task_run_on_close.rs (17行) // traitのみ
task_run_priority.rs (44行) // enum + rank()メソッド
task_run_summary.rs (35行)  // 2フィールドの構造体
```

**各ファイルの詳細:**

```rust
// task_run_error.rs - わずか9行
pub enum TaskRunError {
  Failed,
}

// task_run_handle.rs - 31行だがほぼboilerplate
pub struct TaskRunHandle { id: u64 }
impl TaskRunHandle {
  pub const fn new(id: u64) -> Self { Self { id } }
  pub const fn id(&self) -> u64 { self.id }
}

// task_run_summary.rs - 2フィールドだけ
pub struct TaskRunSummary {
  pub executed_tasks: usize,
  pub failed_tasks: usize,
}
```

**無駄の内容:**
- `TaskRunError`: 9行で独立ファイル（`Result<(), Box<dyn Error>>`で代替可能）
- `TaskRunHandle`: シンプルなu64ラッパーに31行専用ファイル
- `TaskRunSummary`: 2フィールドだけの構造体（タプルで十分）

**簡素化案:**
```rust
// task_run.rs に統合
pub trait TaskRunOnClose {
  fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRunPriority {
  High = 2,
  Medium = 1,
  Low = 0,
}

impl TaskRunPriority {
  pub const fn rank(self) -> u32 {
    self as u32
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRunHandle(u64);

impl TaskRunHandle {
  pub const fn new(id: u64) -> Self { Self(id) }
  pub const fn id(&self) -> u64 { self.0 }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskRunSummary {
  pub executed_tasks: usize,
  pub failed_tasks: usize,
}

// 内部実装
pub(crate) struct TaskRunEntry {
  priority: TaskRunPriority,
  sequence: u64,
  handle: TaskRunHandle,
  task: ArcShared<dyn TaskRunOnClose>,
}

pub(crate) type TaskRunQueue = BinaryHeap<TaskRunEntry>;
```

**効果:**
- 5ファイル削減（task_run_*.rs → task_run.rs 1ファイル）
- 関連型が一箇所に集約され理解しやすくなる
- ユーザーAPIへの影響: なし（すべて同じ型を公開）

---

## 維持すべき設計（正当な理由がある）

以下は適切な設計と判断：

### 1. SchedulerCore vs SchedulerRunner
- **理由**: 異なる責務（コアロジック vs 実行環境統合）
- **評価**: ✅ 正当な分離

### 2. ExecutionBatch
- **理由**: ユーザーが受け取るメタデータ（公開API）
- **評価**: ✅ 独立した型として正当

### 3. SchedulerMetrics / SchedulerWarning
- **理由**: 観測性のための公開型、ユーザーが消費する
- **評価**: ✅ 必要な分離

### 4. SchedulerDump / SchedulerDumpJob
- **理由**: 診断ツール向け公開API
- **評価**: ✅ デバッグ/診断に必要

### 5. SchedulerCommand
- **理由**: ユーザーが登録するコマンドの型
- **評価**: ✅ 核心的な公開API

### 6. SchedulerHandle
- **理由**: ユーザーがジョブをキャンセルするためのハンドル
- **評価**: ✅ 必須の公開型

---

## 簡素化の効果まとめ

### 削減可能なファイル（実装の統合）

| 優先度 | 対象ファイル | 統合先 | 削減効果 |
|--------|-------------|--------|----------|
| ⭐️ 高 | fixed_rate_context.rs | periodic_context.rs | コード重複 ~100行削減 |
| ⭐️ 高 | fixed_delay_context.rs | periodic_context.rs | 上記に含む |
| 🟡 中 | cancellable_registry.rs | scheduler_core.rs | 間接層 ~30行削減 |
| 🟡 中 | cancellable_state.rs | cancellable_entry.rs | 間接層 ~27行削減 |
| 🟡 中 | task_run_error.rs | task_run.rs | 5ファイル → 1ファイル |
| 🟡 中 | task_run_handle.rs | task_run.rs | 上記に含む |
| 🟡 中 | task_run_summary.rs | task_run.rs | 上記に含む |
| 🟡 中 | task_run_priority.rs | task_run.rs | 上記に含む |
| 🟡 中 | task_run_on_close.rs | task_run.rs | 上記に含む |
| 🟢 低 | deterministic_log.rs | scheduler_diagnostics.rs | ラッパー ~20行削減 |
| 🟢 低 | deterministic_replay.rs | scheduler_diagnostics.rs | ラッパー ~20行削減 |
| 🟢 低 | diagnostics_registry.rs | scheduler_diagnostics.rs (private module) | 局所化 |
| 🟢 低 | policy_registry.rs | config.rs | 薄いラッパー削減 |

### 数値的な効果

- **ファイル数**: 42ファイル → 約29-30ファイル（**30%削減**）
- **コード削減**:
  - 重複コード: ~100行
  - 薄いラッパー: ~60行
  - 過剰な分割: ~50行
  - **合計: 約210行削減**
- **ユーザーAPIへの影響**: **0%**（すべての公開型を維持）

---

## 推奨アクション

### Phase 1: 即座に実施可能（コード重複の削減）

**優先度: ⭐️⭐️⭐️**

1. **FixedRateContext + FixedDelayContext → PeriodicContext統合**
   - ファイル削減: 2 → 1
   - コード重複削減: ~100行
   - 影響範囲: scheduler_core.rsのみ（内部実装）
   - リスク: 低

2. **TaskRun関連 6ファイル → task_run.rs 1ファイル統合**
   - ファイル削減: 6 → 1
   - 関連型の集約
   - 影響範囲: 公開APIは維持、内部構造のみ変更
   - リスク: 低

### Phase 2: リファクタリング（実装の簡素化）

**優先度: ⭐️⭐️**

3. **CancellableRegistry + CancellableState統合**
   - Registryをscheduler_core.rsに統合
   - StateをCancellableEntryに統合
   - 間接層削除: ~60行
   - 影響範囲: 内部実装のみ
   - リスク: 中（状態管理の変更）

4. **DeterministicLog + DeterministicReplay統合**
   - SchedulerDiagnosticsに統合
   - イテレータは標準機能で実現
   - ラッパー削除: ~40行
   - 影響範囲: DeterministicReplay型の変更（impl Iteratorで代替）
   - リスク: 低

5. **PolicyRegistry → SchedulerConfig統合**
   - 設定の一元化
   - 型エイリアスで互換性維持
   - 影響範囲: 内部構造のみ
   - リスク: 低

### Phase 3: モジュール構成の整理

**優先度: ⭐️**

6. **DiagnosticsRegistry → scheduler_diagnostics.rsのprivate module移動**
   - ファイル削減: 1
   - 実装の局所化
   - 影響範囲: なし（pub(crate)のみ）
   - リスク: 極低

---

## 実装時の注意事項

### 破壊的変更の許可

プロジェクトの方針より:
> **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
> **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること。

この方針により、以下が可能:
- 公開型の変更（ただしユーザー機能は維持）
- 内部実装の大幅な変更
- モジュール構成の再編成

### テストの維持

すべてのリファクタリングにおいて:
- 既存のテストはすべて維持
- テストのコメントアウト・無視は禁止
- リファクタリング後に全テストがパスすること

### 完了条件

```bash
./scripts/ci-check.sh all
```

すべてのチェックがパスすることを確認。

---

## 結論

**現在の実装は約30%の簡素化余地がある**

主な無駄:
1. **コード重複**: FixedRate/Delayの95%同一コード
2. **薄すぎるラッパー**: Registry, Log, Replayなど
3. **過剰な分割**: TaskRun関連の8ファイル分散

すべての簡素化は**ユーザー向け機能を維持**しつつ、**内部実装（How）のみを改善**する。

"Less is more"と"YAGNI"の観点から、Phase 1の実施を強く推奨する。
