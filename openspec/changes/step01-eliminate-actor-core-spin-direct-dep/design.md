## Context

`actor-core` 内での `spin` クレート直接利用は実態調査の結果 **`spin::Once<T>` の利用 2 ファイル のみ**:

- `modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs:23`: `use spin::Once;`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs:10`: `use spin::Once;`、3 フィールドで `Once<T>` を使用（`instrumentation`、`invoker`、`actor`）

利用パターンは **write-once + lock-free read**: 一度だけ初期化し、その後はロックなしで読み出す。`spin::Once` は no_std 環境でこのセマンティクスを提供する標準的な選択。

`actor-core/Cargo.toml:35` の `spin = { workspace = true, default-features = false, features = ["mutex", "spin_mutex", "once"] }` の features には `"mutex"` と `"spin_mutex"` が含まれているが、ソースコードでの利用は `"once"` のみ。features 指定が実態に対して過剰。

`utils-core` には現在 `Once<T>` 相当の抽象が存在しない:
- `SpinSyncMutex<T>`、`SpinSyncRwLock<T>`、`SharedLock<T>`、`SharedRwLock<T>` はあり
- `Once<T>` または `OnceLock<T>` 相当はなし

`std::sync::OnceLock<T>` は Rust 1.70+ stable だが std 限定。`actor-core` は no_std クレート。`core::sync::atomic::Once` のような構造は core にも存在しない。

## Goals / Non-Goals

**Goals:**
- `actor-core` の production code から `spin` クレート直接 use を撤去
- `actor-core/Cargo.toml` から `spin` 直接依存を完全削除
- `utils-core` に Once 系の 3 段構造を新設し、既存の `LockDriver` / `SpinSyncMutex` / `SharedLock` パターンと相似形にする:
  - driver trait `OnceDriver<T>`
  - backend 実装 `SpinOnce<T>`（`spin::Once<T>` の thin wrapper）
  - 公開抽象 `SyncOnce<T>`（actor-\* の依存先、`spin::Once<T>` 相当の write-once + lock-free read セマンティクスを no_std 対応で提供）
- `actor-lock-construction-governance` spec への準拠状態を回復し、spin 固有 Scenario を追加して検査カバレッジを広げる
- 既存の機能性・パフォーマンス（lock-free read）を維持

**Non-Goals:**
- 他 actor-* クレート（cluster-\*, remote-\*, stream-\*, persistence-\*, actor-adaptor-std 等）の `spin` 直接依存調査・撤去（hand-off、必要なら別 change）
- `utils-core` 内の `spin` 利用見直し（backend 実装層として spec 例外で許可、本 change 対象外）
- `Once` 以外の `spin` 抽象（`Mutex`、`RwLock`、`Lazy` 等）の `utils-core` への追加（YAGNI、現在 `actor-core` では使われていない）
- `DefaultOnce<T>` のような feature 切替による driver 選択機構の導入（将来拡張用、本 change では `SyncOnce<T>` が常に `SpinOnce<T>` を保持する単段構成）

（補足）`actor-core/Cargo.toml` の `[dev-dependencies]` 側には `spin` 直接依存は存在しないことを起案時に確認済みであり、tasks 1.2 で最終再確認するのみ。Non-Goal ではない。

## Decisions

### Decision 1: 既存の `LockDriver` / `SpinSyncMutex` / `SharedLock` パターンに倣った 3 段構造を `utils-core` に新設する

**選択**: `utils-core` に以下の 3 要素を追加し、既存の同期プリミティブ抽象と対応関係を持たせる:

| 層 | Once 系（新設） | Mutex 系（既存） | Rwlock 系（既存） | 責務 |
|---|---|---|---|---|
| driver trait | `OnceDriver<T>` | `LockDriver<T>` | `RwLockDriver<T>` | backend 契約 |
| backend 実装 | `SpinOnce<T>` | `SpinSyncMutex<T>` | `SpinSyncRwLock<T>` | `spin` クレートを直接使う唯一の場所 |
| 公開抽象 | `SyncOnce<T>` | `SharedLock<T>` | `SharedRwLock<T>` | actor-* などの上位層が依存する先 |

`actor-core` は `SpinOnce`（backend 実装）ではなく `SyncOnce`（抽象）に依存する。これにより `actor-lock-construction-governance` Requirement の「actor-\* は primitive lock crate を直接 use しない」原則を atomic once 系まで一貫して適用できる。

**根拠**:
- 既存パターン（`LockDriver`/`SpinSyncMutex`/`SharedLock`）と完全に対応関係を持つため、命名・配置・責務が一意に定まる
- spec `actor-lock-construction-governance` の例外条項「`utils-core` 内の backend 実装層は primitive lock crate 直接 use を許可」が `SpinOnce` にも自然に適用される
- `actor-core` から `spin` への direct edge が消える
- 将来 std 環境向けに `StdOnce<T>`（`std::sync::OnceLock<T>` backend）を追加したくなっても、driver trait が既にあるので `DefaultOnce<T>` の feature 切替で差し替え可能（ただし本 change では追加しない）

**代替案と却下理由**:
- 案 A: `actor-core` で `spin::Once` を別パターンに置換（例: `SharedLock<Option<T>>`）→ lock-free read セマンティクスが失われ、ホットパス（`mailbox/base.rs` の `instrumentation`/`invoker`/`actor`）で毎回ロック取得が発生し性能劣化。不採用
- 案 B: `actor-core` で `spin::Once` を許容例外として spec に追加 → 自分で作った規約に例外を追加するのは「割れ窓」化、本 change の趣旨と反する
- 案 C: `utils-core` に単一型（`SyncOnce<T>` のみで内部が `spin::Once`）だけ追加 → mutex / rwlock との対応関係が崩れ、将来 backend を切り替える余地も closed になる。既存の 3 段構造と揃える方が拡張余地・責務分離の両面で優位
- 案 D: `core::sync::atomic` ベースで独自 Once 実装 → `spin::Once` を再発明する手間、unsafe レビューコスト大、既存の堅牢な実装をラップする方が安全

### Decision 2: `SpinOnce<T>` の内部は `spin::Once<T>` の thin wrapper とする

**選択**: `SpinOnce<T>` は `spin::Once<T>` を内部に保持する薄いラッパーとして `utils-core/src/core/sync/spin_once.rs` に配置する。`SpinSyncMutex<T>` と同様の構造。

**根拠**:
- `spin::Once` は no_std 対応で実装が成熟
- 独自実装は `AtomicBool` + `UnsafeCell<MaybeUninit<T>>` の組み合わせで unsafe 多用、レビューコスト大
- 性能特性（lock-free read）を完全維持
- 既存の `SpinSyncMutex` / `SpinSyncRwLock` が `spin::Mutex` / `spin::RwLock` のラップである構造と一貫

**代替案と却下理由**:
- 案 A: 独自実装 → 不採用理由は上記
- 案 B: `portable-atomic-util::OnceLock` を内部で使う → 依存経路が複雑化、`portable-atomic-util` に該当 API があるかの確認コストも本 change の範囲を超える

### Decision 3: `OnceDriver<T>` trait と API 形状

**選択**: driver trait は以下の最小契約を定める。

```rust
pub trait OnceDriver<T>: Sized {
  fn new() -> Self;
  fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T;
  fn get(&self) -> Option<&T>;
  fn is_completed(&self) -> bool;
}
```

`SpinOnce<T>: OnceDriver<T>` を実装する。`SyncOnce<T>` は `OnceDriver<T>` trait object に直接依存せず、`LockDriver` / `SharedLock` と同様に `SyncOnceBackend` のような crate-internal trait を介して `ArcShared<dyn SyncOnceBackend<T>>` を保持する形も考えられるが、Once は 1 度書いたら読むだけなので「共有」自体が不要なケースが多い。そのため **`SyncOnce<T>` は直接 `SpinOnce<T>`（あるいは feature で選ばれた driver）を保持する単段構成とし、`ArcShared` 層を介さない** 設計を採用する。

**本 change の具体化**:
- `SyncOnce<T>` の内部型は `SpinOnce<T>` を直接保持する構造にする（`DefaultMutex` のような feature 切替は本 change では導入しない。将来拡張時に追加）
- 公開 API は `OnceDriver<T>` trait と同じく `new` / `call_once` / `get` / `is_completed` を `SyncOnce<T>` の inherent method として提供

```rust
impl<T> SyncOnce<T> {
  pub const fn new() -> Self;
  pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T;
  pub fn get(&self) -> Option<&T>;
  pub fn is_completed(&self) -> bool;
}
```

`actor-core` 側の置換は `use spin::Once;` → `use fraktor_utils_core_rs::core::sync::SyncOnce;`、および型名 `Once<T>` → `SyncOnce<T>` の機械的変更で済む。

**根拠**:
- `spin::Once<T>` の主要 API と互換にすることで置換コストを最小化
- driver trait を別途用意しておけば、将来 `StdOnce`（`std::sync::OnceLock` backend）や `DebugOnce`（instrumentation 付き）を追加する余地が残る
- `ArcShared` 層を入れないことで `SharedLock` より軽量（Once は mutation しないため shared 不要、`&SyncOnce<T>` を各所に渡すだけで十分）

**代替案と却下理由**:
- 案 A: `SyncOnce<T>` も `SharedLock` と同じく `ArcShared<dyn SyncOnceBackend<T>>` を持つ → Once の利用パターンでは shared 所有の必要がなく、実質 overhead のみ増える。不採用
- 案 B: `OnceCell` 風の `get_or_init` API 中心 → 現利用箇所（`spin::Once::call_once`）と API 形状が異なり置換コスト増。YAGNI

### Decision 4: spec には spin 固有 Scenario を MODIFIED で追加

**選択**: `actor-lock-construction-governance` spec の Requirement「actor-\* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない」に、`spin` 固有の Scenario を MODIFIED で追加する。

**根拠**:
- 既存 Requirement は本文で `critical-section、spin、parking_lot` を列挙しているが、Scenario は `critical-section` のみを検査する形になっていた
- 本 change で `spin` 直接依存を撤去するため、同じ形式で `spin` 用 Scenario を追加しておけば spec の検査カバレッジと本 change の成果が 1:1 に対応する
- `SyncOnce<T>` が `utils-core` 側の正式な write-once + lock-free read 抽象であることを Scenario レベルで明示できる
- OpenSpec 運用上、change には最低 1 つの delta が必須（validation 要件）。スコープ内で自然な delta として spin Scenario が最適

**代替案と却下理由**:
- 案 A: spec を変更せず本 change は実装のみとする → OpenSpec strict validation に違反（delta が 1 つも無い change は reject される）。また、将来他 actor-* crate で spin 直接依存が再発した場合に Scenario で捕捉できない
- 案 B: 新規 Requirement として ADDED する → 既存 Requirement が既に本文で spin を対象としており、重複する。Scenario 追加が最小差分

### Decision 5: `spin` 直接依存行の完全削除と clippy safety net 追加

**選択**: `actor-core/Cargo.toml:35` の `spin = { ..., features = ["mutex", "spin_mutex", "once"] }` 行を **完全削除** する。さらに `actor-core/clippy.toml` の `disallowed-types` に `spin::Once` エントリを追加する（`replacement = "fraktor_utils_core_rs::core::sync::SyncOnce"`）。

**根拠**:
- `spin` 自体への依存が消えるため `features` 指定も全て不要（部分削減ではなく行ごと削除が最小差分）
- 仮に他箇所で `spin::Once` 以外の利用があっても、本 change の調査範囲（`use spin::` および `spin::` の grep）で発見されていない。tasks フェーズの `cargo build` で実証
- `clippy.toml` の `disallowed-types` には既に `spin::Mutex` / `spin::RwLock` エントリがある。`spin::Once` も同列に追加しておくことで、将来 `spin` を transitive にでも再導入した場合に、`SyncOnce` ではなく `spin::Once` を直接使う書き方を lint が捕捉できる（safety net の二重化）

## Risks / Trade-offs

- **[Risk] `SyncOnce` の `Send`/`Sync` トレート境界** → Mitigation: `spin::Once<T>` が `T: Send + Sync` で `Send + Sync` を実装するのと同じ制約を継承。`actor-core` 側の `Once<MailboxInstrumentation>`、`Once<MessageInvokerShared>`、`Once<WeakShared<ActorCell>>` はすべて `Send + Sync` を満たす型なので問題なし
- **[Risk] `SpinOnce::new()` / `SyncOnce::new()` を `const fn` にできるか** → Mitigation: `spin::Once::new()` は `const fn` なので、`SpinOnce::new()` も `const fn { Self(spin::Once::new()) }` で実現可能。`SyncOnce::new()` も同じく `const fn { Self { inner: SpinOnce::new() } }` で連鎖的に `const fn` を維持できる
- **[Risk] `actor-core` の他箇所で実は `spin` を別途利用している可能性** → Mitigation: tasks 1.1 で `Grep` 全数調査、`Cargo.toml` の `spin` 直接依存削除後に `cargo build` で compile 時エラーとして検出（依存経路が消えると `use spin::*` は unresolved import になる）
- **[Trade-off] `utils-core` の依存に `spin` が引き続き必要** → 受容: spec 例外として `utils-core` の backend 実装層は許可されている。本 change の趣旨は「`actor-core` 単位での違反解消」

## Migration Plan

本 change はライブラリ内部実装の置換であり、ダウンストリーム移行手順は不要。

1. **Phase 1a**: `utils-core` に `OnceDriver<T>` trait を新設（`once_driver.rs`）
2. **Phase 1b**: `utils-core` に `SpinOnce<T>` backend 実装を新設（`spin_once.rs`、`OnceDriver<T>` を impl）
3. **Phase 1c**: `utils-core` に `SyncOnce<T>` 公開抽象を新設（`sync_once.rs`、`SpinOnce<T>` を内部保持）
4. **Phase 2a**: `actor-core` の `spin::Once` 利用 2 ファイル（`coordinated_shutdown.rs`、`mailbox/base.rs`）を `SyncOnce` に置換
5. **Phase 2b**: `actor-core` 内の docstring 残存言及（`mailbox_shared_set.rs` 等）を `SyncOnce` に置換
6. **Phase 3**: 個別ビルド確認（`cargo build -p fraktor-actor-core-rs`、`cargo test -p fraktor-actor-core-rs --features test-support`）
7. **Phase 4a**: `actor-core/Cargo.toml` から `spin` 直接依存削除
8. **Phase 4b**: `actor-core/clippy.toml` の `disallowed-types` に `spin::Once` を追加（safety net 強化、将来の再発防止）
9. **Phase 5**: `cargo tree -p fraktor-actor-core-rs --no-default-features --depth 1` で `spin` が消えたことを確認
10. **Phase 6**: `./scripts/ci-check.sh ai all` で workspace 全体確認
11. **Phase 7**: `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 4 を「解消済み」に更新

ロールバックは git revert で完結する。

## Open Questions

- なし（必要な設計判断は本 design で確定済み）
