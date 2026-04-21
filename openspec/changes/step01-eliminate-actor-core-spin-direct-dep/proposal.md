## Why

直前 archive された change `retire-actor-core-test-support-critical-section-impl`（PR #1607/#1608）で `actor-lock-construction-governance` spec の Requirement「`actor-*` の `Cargo.toml` は primitive lock crate を non-optional な直接依存として宣言してはならない」を確立した。しかし `actor-core/Cargo.toml:35` に `spin = { workspace = true, default-features = false, features = ["mutex", "spin_mutex", "once"] }` が残っており、これは **本 spec の自分自身による違反** 状態。

実態調査により、`actor-core/src` 内での `spin` クレート直接利用は **`spin::Once` の 2 箇所のみ**（`coordinated_shutdown.rs:23`、`mailbox/base.rs:10`）。`spin::Mutex` の利用はなく、`Cargo.toml` の features 指定（`"mutex", "spin_mutex"`）は実態に対して過剰。

「自分で追加したガバナンスに自分で違反している」状態を解消し、ガバナンス完全準拠状態にする。

## What Changes

既存の `LockDriver` / `SpinSyncMutex` / `SharedLock` 3 段構造と相似形になるよう、`utils-core` 側に同じ責務分離の 3 要素を追加する:

| 層 | Once 系（本 change 新設） | Mutex 系（既存） |
|---|---|---|
| driver trait | `OnceDriver<T>` | `LockDriver<T>` |
| backend 実装 | `SpinOnce<T>`（`spin::Once<T>` を直接使う唯一の場所） | `SpinSyncMutex<T>` |
| 公開抽象 | `SyncOnce<T>`（actor-\* が依存する先） | `SharedLock<T>` |

- **新 trait**: `utils-core` に `OnceDriver<T>` trait を追加（`new`/`call_once`/`get`/`is_completed`）
- **新 backend 実装**: `utils-core` に `SpinOnce<T>` を追加（`spin::Once<T>` の thin wrapper、`OnceDriver<T>` を impl）
- **新公開抽象**: `utils-core` に `SyncOnce<T>` を追加（`SpinOnce<T>` を内部保持する単段構成、write-once + lock-free read セマンティクスを維持）
- **置換**: `actor-core` の `spin::Once` 利用 2 箇所を `SyncOnce` に置換
  - `modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs:23`
  - `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:10`（`instrumentation`, `invoker`, `actor` の 3 フィールド）
- **依存削除**: `modules/actor-core/Cargo.toml:35` から `spin = { workspace = true, ... }` 行を完全削除
- **spec 更新**: `actor-lock-construction-governance` の既存 Requirement に spin 固有 Scenario を MODIFIED で追加（validation 要件を兼ねる）
- **ドキュメント更新**: `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 4 を「解消済み」に更新

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `actor-lock-construction-governance`: 既存 Requirement「actor-\* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない」に **spin 固有 Scenario を追加**（Scenario 数が従来の `critical-section` 検査のみ → `spin` 検査も含む形に拡張）。Requirement 本文（文言）は変更しない。

## Impact

### 影響を受けるコード

- `modules/utils-core/src/core/sync/once_driver.rs`（新規ファイル、`OnceDriver<T>` trait 定義）
- `modules/utils-core/src/core/sync/spin_once.rs`（新規ファイル、`SpinOnce<T>` backend 実装）
- `modules/utils-core/src/core/sync/sync_once.rs`（新規ファイル、`SyncOnce<T>` 公開抽象）
- `modules/utils-core/src/core/sync.rs`（`OnceDriver` / `SpinOnce` / `SyncOnce` の `pub use` 追加）
- `modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs`（`spin::Once` → `SyncOnce` 置換）
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`（同、および docstring 内の `spin::Once::get()` 言及 2 箇所を更新）
- `modules/actor-core/src/core/kernel/system/shared_factory/mailbox_shared_set.rs`（docstring 内の `spin::Once<T>` 言及を `SyncOnce<T>` に更新）
- `modules/actor-core/Cargo.toml`（`spin` 直接依存削除）
- `modules/actor-core/clippy.toml`（`disallowed-types` に `spin::Once` を追加し safety net を強化）
- `docs/plan/2026-04-21-actor-core-critical-section-followups.md`（残課題 4 を解消済みに更新）

### 影響を受けない範囲

- `actor-core` の他のソースコード（`spin::Once` の置換に伴う public API 変更なし）
- `utils-core` 内の `spin` 利用（backend 実装層として spec 例外、`spin_sync_mutex.rs` 等）
- 他 actor-* クレート（cluster-*, remote-*, stream-*, persistence-*）の `Cargo.toml`（本 change 対象外、別途 hand-off で確認）
- `actor-core` の `[dev-dependencies]` の `critical-section`（前 change で確定済み）

### 依存関係

- `actor-core` から `spin` クレートへの直接依存が完全に消える（`utils-core` 経由の transitive で `spin` は `utils-core` のみが利用）
- workspace 全体での `spin` 利用ポリシー: `utils-core` のみで使用、actor-* は `utils-core` 抽象経由

### リスク

- **`SyncOnce<T>` の API 設計**: `spin::Once<T>` の API（`call_once`、`get` 等）と互換性を持たせる必要。design.md で詳細設計
- **パフォーマンス影響**: `spin::Once` の lock-free read セマンティクスを `SyncOnce` でも維持する必要。内部実装は `spin::Once` のラップが最も安全（性能影響なし）
- **`actor-core/Cargo.toml` の features 指定**: 現在 `["mutex", "spin_mutex", "once"]` だが、削除すれば全 feature 不要

### 後続 change（hand-off）

- 他 actor-* クレートの `Cargo.toml` で `spin` 直接依存がないか調査（必要なら別 change）
- `actor-core/Cargo.toml` の `[dev-dependencies]` には `spin` 直接依存は無いことを起案時に確認済み（残作業なし）
