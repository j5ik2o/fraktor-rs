## Why

`modules/actor/src/std` に no_std で動作可能なロジック・型定義が多数残っている。調査の結果、44ファイル中約28ファイル（約6割）がプラットフォーム依存なしであり、core/ への移設が可能。

加えて以下の設計負債がある：

1. **CircuitBreaker** — 状態機械ロジック全体が `std/` にあるが、唯一の std 依存は `std::time::Instant` のみ。Clock Port を抽出すれば core/ で動作する
2. **DispatchExecutor の重複** — core と std に同名 trait が存在し、adapter で橋渡しする冗長な構造。実際の std 実装（TokioExecutor, ThreadedExecutor）は両方とも Sync なので core trait を直接実装可能
3. **`quickstart` メソッド** — Pekko にも protoactor にもない fraktor 独自の命名。ドメイン用語として不適切。Pekko の `ActorSystem.apply(behavior, name)` に倣い、`new()` がデフォルト設定で動く設計に変更する
4. **CoordinatedShutdown の型定義** — error/id/phase/reason はプラットフォーム依存ゼロだが std/ に配置されている

## What Changes

### A. CircuitBreaker の core 移設

- `core/pattern/clock.rs` に最小限の `Clock` Port trait を新設
- `std/pattern/` の CircuitBreaker 関連ファイル（状態機械、エラー型、Shared ラッパー）を `core/pattern/` へ移設
- `std/pattern/std_clock.rs` に `std::time::Instant` ベースの Clock 実装を残す
- CircuitBreaker を `Clock` trait でジェネリック化

### B. DispatchExecutor の統合（DRY）

- **BREAKING** `std/dispatch/dispatcher/base.rs` の std 版 `DispatchExecutor` trait を廃止
- **BREAKING** `std/dispatch/dispatcher/dispatch_executor_adapter.rs`（ブリッジ）を削除
- `TokioExecutor` / `ThreadedExecutor` が `core::dispatch::DispatchExecutor` を直接実装するよう変更
- `DispatcherConfig` の executor 受け取りを core trait ベースに統一

### C. quickstart 廃止 → new() デフォルト化

- **BREAKING** `ActorSystem::quickstart()` / `quickstart_with()` を削除
- `ActorSystem::new(&props)` が `#[cfg(feature = "tokio-executor")]` 有効時にデフォルト設定（TickDriver + Dispatcher）を自動構成
- **BREAKING** `TickDriverConfig::tokio_quickstart()` → `TickDriverConfig::default()`（feature gate）
- **BREAKING** `DispatcherConfig::tokio_auto()` → `DispatcherConfig::default()`（feature gate）
- `std::system::ActorSystem` ラッパー型の簡素化（quickstart 固有ロジックの除去）
- カスタマイズは既存の `ActorSystem::new_with_config(&props, &config)` で対応

### D. CoordinatedShutdown 型の core 移設

- `std/system/coordinated_shutdown_error.rs` → `core/system/`
- `std/system/coordinated_shutdown_id.rs` → `core/system/`
- `std/system/coordinated_shutdown_phase.rs` → `core/system/`
- `std/system/coordinated_shutdown_reason.rs` → `core/system/`
- 実行ロジック（`coordinated_shutdown.rs`）は tokio 依存のため std/ に残す

## Capabilities

### New Capabilities

- `circuit-breaker-core-port`: CircuitBreaker を core/ に移設し、Clock Port で時刻依存を抽象化する
- `dispatch-executor-unification`: DispatchExecutor trait を core に統一し、std 版 trait と adapter を廃止する
- `actor-system-default-config`: quickstart を廃止し、`ActorSystem::new()` でデフォルト構成を提供する
- `coordinated-shutdown-core-types`: CoordinatedShutdown の型定義を core/ に移設する

### Modified Capabilities

なし

## Impact

- 影響コード: `modules/actor/src/std/`, `modules/actor/src/core/`, `modules/actor/examples/`, `modules/actor/benches/`, `modules/cluster/examples/`, `modules/remote/examples/`, `modules/remote/tests/`
- 影響 API: `crate::std::pattern::*`（移設）、`crate::std::dispatch::DispatchExecutor`（廃止）、`crate::std::system::ActorSystem::quickstart*`（廃止）、`crate::std::system::coordinated_shutdown_*`（移設）
- 破壊的変更: すべての capability で破壊的変更あり（リリース前フェーズのため許容）

## Dependencies and Order

```
A (CircuitBreaker)      ← 独立して実施可能
D (CoordinatedShutdown) ← 独立して実施可能
B (DispatchExecutor)    ← C に先行すべき（DispatcherConfig の統一が C の前提）
C (quickstart 廃止)     ← B の完了後に実施
```

推奨実施順: A → D → B → C

## Non-goals

- `core/` 内の既存設計の変更（今回は std → core の移設のみ）
- `std::typed::behaviors.rs` の移設（tracing 依存があり core 移設不可）
- `std::system::base.rs` ラッパー型の完全廃止（quickstart 除去のみ。ラッパーの是非は別途検討）
- CoordinatedShutdown の実行ロジックの Port 化（フェーズグラフは std 専用で十分）
- ActorSystemSetup のような合成可能な設定パターン（YAGNI — 必要になったら追加）
