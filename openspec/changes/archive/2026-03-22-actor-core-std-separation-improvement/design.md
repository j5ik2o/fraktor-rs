## Context

`modules/actor/src/std` にプラットフォーム依存のないロジック・型定義が多数混在している。調査の結果、44ファイル中約28ファイルが no_std 互換であり、core/ への移設が可能。加えて DispatchExecutor trait の重複、`quickstart` という不適切な命名、CoordinatedShutdown 型の配置問題がある。

現在の依存構造:

```
std/dispatch/dispatcher/
├── base.rs (std版 DispatchExecutor trait: Send + 'static)
├── dispatch_executor_adapter.rs (StdSyncMutex ブリッジ)
├── dispatch_executor/
│   ├── tokio_executor.rs (impl std::DispatchExecutor)
│   └── thread_executor.rs (impl std::DispatchExecutor)
└── dispatcher_config.rs (tokio_auto メソッド)

std/pattern/
├── circuit_breaker.rs (状態機械、std::time::Instant 依存)
├── circuit_breaker_state.rs, _call_error.rs, _open_error.rs
└── circuit_breaker_shared.rs (ArcShared<RuntimeMutex<CB>>)

std/system/
├── base.rs (CoreActorSystem ラッパー + quickstart)
├── coordinated_shutdown.rs (tokio 依存実行ロジック)
└── coordinated_shutdown_{error,id,phase,reason}.rs (プラットフォーム依存なし)
```

## Goals / Non-Goals

**Goals:**

- `std/` の責務を「core の Port に対する std/tokio adapter 実装」に限定する
- CircuitBreaker の状態機械ロジックを core/ に移設し、no_std 環境でも利用可能にする
- DispatchExecutor trait を core に統一し、冗長な adapter 層を廃止する
- `quickstart` を廃止し、Pekko に倣った `new()` デフォルト構成を提供する
- CoordinatedShutdown のプラットフォーム非依存な型定義を core/ に移設する

**Non-Goals:**

- `std::typed::behaviors.rs` の移設（tracing 依存により core 移設不可）
- `std::system::base.rs` ラッパー型の完全廃止（quickstart 除去のみ）
- CoordinatedShutdown 実行ロジックの Port 化
- ActorSystemSetup のような合成可能な設定パターンの導入（YAGNI）
- core/ 内の既存設計の変更

## Decisions

### D1: CircuitBreaker の Clock 抽象化に既存の MonotonicClock を使わず新設する

**選択:** `core/pattern/clock.rs` に CircuitBreaker 専用の `Clock` trait を新設する

**代替案:**
- (a) `fraktor_utils_rs::core::time::MonotonicClock` を再利用 → `TimerInstant` 型が scheduler 向けに設計されており、`Duration` の引き算に使うには不適切
- (b) `Box<dyn Fn() -> Instant>` のまま型パラメータ化 → `Instant` 型自体が no_std で定義できない

**理由:** CircuitBreaker に必要な操作は `now()` と `elapsed_since()` の2つだけ。scheduler の `MonotonicClock` は `TimerInstant` を返すが、CircuitBreaker は `Duration` の比較のみ必要。責務が異なるため分離する。

**Clock trait 設計:**

```rust
// core/pattern/clock.rs
pub trait Clock: Send + Sync {
    type Instant: Copy + Ord + Send + Sync;
    fn now(&self) -> Self::Instant;
    fn elapsed_since(&self, earlier: Self::Instant) -> Duration;
}
```

### D2: DispatchExecutor を core trait に統一し std 版を廃止する

**選択:** `TokioExecutor` と `ThreadedExecutor` が `core::dispatch::DispatchExecutor` を直接実装する

**代替案:**
- (a) 現状維持（std trait + adapter） → 不要な indirection、DRY 違反
- (b) core trait の bound を `Send + 'static` に緩和 → Sync を外すと core 側の安全性保証が弱まる

**理由:** `TokioExecutor` は `tokio::runtime::Handle`（Sync）を保持、`ThreadedExecutor` は `Option<String>`（Sync）を保持。両方とも Sync を満たすため、core trait を直接実装可能。`supports_blocking()` のデフォルト実装（`true`）もそのまま使える。

**変更の影響:**
- `DispatchExecutorAdapter` を削除
- `DispatcherConfig` の executor 受け取りを `Box<dyn core::DispatchExecutor>` に変更
- `StdSyncMutex` による外部ロックは `DispatchExecutorRunner` の `RuntimeMutex` で代替（既に core 側に存在）

### D3: quickstart 廃止 → ActorSystem::new() のデフォルト構成

**選択:** `ActorSystem::new(&props)` が feature gate に応じてデフォルト設定を自動構成する

**代替案:**
- (a) `ActorSystem::with_tokio(&props)` のようなランタイム明示メソッド → feature gate で1つしかないなら冗長
- (b) Builder パターン → 現時点では YAGNI

**理由:** Pekko の `ActorSystem.apply(behavior, name)` はデフォルト設定で動作する。fraktor-rs でも `new()` がそのまま動くべき。カスタマイズは既存の `new_with_config()` で対応。

**実装方針:**
- `ActorSystem::new(&props)` 内で `#[cfg(feature = "tokio-executor")]` 時に `TickDriverConfig::default()` + `DispatcherConfig::default()` を自動構成
- `TickDriverConfig::default()` が tokio-executor 有効時に 10ms resolution のデフォルトを返す
- `DispatcherConfig::default()` が tokio-executor 有効時に現在の Tokio runtime handle を自動検出
- `quickstart()` / `quickstart_with()` / `tokio_quickstart()` / `tokio_auto()` を削除

### D4: CoordinatedShutdown 型の core 移設は feature gate を外す

**選択:** error/id/phase/reason を core/ に移設し、`#[cfg(feature = "tokio-executor")]` gate を外す

**代替案:**
- (a) std/ に残して feature gate 維持 → プラットフォーム依存なしなのに feature gate は不自然
- (b) 型定義と実行ロジックを両方 core に → 実行ロジックは tokio 依存のため不可

**理由:** 型定義（`CoordinatedShutdownId`, `CoordinatedShutdownPhase`, `CoordinatedShutdownReason`, `CoordinatedShutdownError`）はすべてプラットフォーム依存ゼロ。core/ に置くことで no_std 環境でも型レベルの参照が可能になる。実行ロジック（`CoordinatedShutdown` 本体）は `tokio::spawn` / `tokio::time::timeout` に依存するため std/ に残す。

### D5: CircuitBreaker を Clock trait でジェネリック化する

**選択:** `CircuitBreaker<C: Clock>` として型パラメータ化する

**代替案:**
- (a) `Box<dyn Clock>` でトレイトオブジェクト → 動的ディスパッチのコスト、no_std での Box 依存
- (b) 現在の `Box<dyn Fn() -> Instant>` を維持 → `Instant` 型が no_std で使えない

**理由:** ジェネリクスにより静的ディスパッチが保証され、no_std でも Box 不要。`CircuitBreakerShared<C: Clock>` も同様にジェネリック化。std/ では `type StdCircuitBreaker = CircuitBreaker<StdClock>` のような型エイリアスで利便性を提供。

## Risks / Trade-offs

**[R1] CircuitBreaker のジェネリック化による API 複雑化** → std/ で `type StdCircuitBreaker = CircuitBreaker<StdClock>` を提供し、ユーザーは型パラメータを意識せずに済む。`CircuitBreakerShared` も同様にエイリアス化。

**[R2] DispatchExecutor 統合時の DispatcherConfig 変更波及** → `DispatcherConfig::from_executor()` のシグネチャ変更は `std/` 内部と examples に限定。`core::DispatchExecutor` を直接受け取る形に変更。

**[R3] quickstart 廃止による既存 examples の破壊** → 13ファイルの import/呼び出し変更が必要。ただし `quickstart()` → `new()` への機械的置換で対応可能。

**[R4] Default trait 実装の feature gate 依存** → `TickDriverConfig::default()` や `DispatcherConfig::default()` が feature gate で異なる実装を持つ。`#[cfg]` の誤用によるコンパイルエラーのリスクがあるが、CI で全 feature 組み合わせをテストすることで軽減。
