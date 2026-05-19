## Context

`remove-shared-wrapper-factories` で actor runtime の shared wrapper 構築は direct construction に回帰した。95 箇所で `SpinSyncMutex<_>` / `SpinSyncRwLock<_>` がハードコードされている。テスト時に再入検知を有効にしたいが、Port & Adapter 構成の制約により core 層は std に依存できない。

既存の `DebugSpinSyncMutex`（utils-adaptor-std）は `std::thread::current().id()` に依存するが、再入検知自体は `AtomicBool` / `AtomicU8` だけで実現でき、std は不要。utils-core に `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` として新設し、名前衝突を回避する。

## Goals / Non-Goals

**Goals:**
- utils-core に no_std 互換の `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` を実装する
- `DefaultMutex<T>` / `DefaultRwLock<T>` type alias を utils-core に導入し、`debug-locks` feature で切替可能にする
- production code の `SpinSyncMutex` / `SpinSyncRwLock` 直書きを type alias に置換する
- `cargo test` 時に dev-dependencies 経由で `debug-locks` が自動有効になる構成にする

**Non-Goals:**
- runtime injection の再導入
- `ActorSystem<B>` 等の public generic type parameter 導入
- `StdSyncMutex` への切替を今回実装すること
- utils-adaptor-std の既存 `DebugSpinSyncMutex` / `DebugSpinSyncRwLock` を削除すること

## Decisions

### 1. no_std 互換の CheckedSpinSyncMutex は AtomicBool ベースで再入検知する

`CheckedSpinSyncMutex` は内部に `SpinSyncMutex` と `AtomicBool`（ロック保持中フラグ）を持つ。`lock()` 時に `AtomicBool::swap(true)` して、swap 前が既に `true` なら `panic!("re-entrant lock detected")` する。Guard の `Drop` で `false` に戻す。

`std::thread::current().id()` は使わないため、panic メッセージにスレッド ID は含まれない。スタックトレースで十分特定可能。

代替案:
- `std::thread` を使う: no_std で使えないため不採用
- `AtomicU64` でスレッド ID を保存: no_std でスレッド ID を取得する portable な手段がないため不採用

### 2. CheckedSpinSyncRwLock は AtomicU8 ベースで write 再入と read→write 昇格の両方を検知する

`CheckedSpinSyncRwLock` は内部に `SpinSyncRwLock` と `AtomicU8`（ロック状態フラグ: `0=free / 1=read / 2=write`）を持つ。`write()` 時に状態が `free` 以外なら panic する（write 再入も read 保持中の write も検知）。`read()` 時に状態が `write` なら panic する（write 保持中の read を検知）。`read()` → `read()` の再入は `spin::RwLock` がデッドロックしないため検知しない。

代替案:
- `AtomicBool` 1 つで write だけ検知: read 保持中の write 再入（デッドロック原因）を見逃すため不採用
- 現行 utils-adaptor-std の DebugSpinSyncRwLock（検知なし）をそのまま移す: 再入検知を追加する価値があるため不採用

### 3. type alias は utils-core の `core::sync` module に置く

```rust
#[cfg(not(feature = "debug-locks"))]
pub type DefaultMutex<T> = SpinSyncMutex<T>;
#[cfg(feature = "debug-locks")]
pub type DefaultMutex<T> = CheckedSpinSyncMutex<T>;

#[cfg(not(feature = "debug-locks"))]
pub type DefaultRwLock<T> = SpinSyncRwLock<T>;
#[cfg(feature = "debug-locks")]
pub type DefaultRwLock<T> = CheckedSpinSyncRwLock<T>;
```

utils-core はすべての上位クレートの依存元なので、type alias が全クレートから見える。

代替案:
- actor-core に置く: actor-core 以外（cluster-core, persistence-core 等）にも call site があるため不採用
- 各クレートに個別に置く: 重複するため不採用

### 4. dev-dependencies で feature を有効化する

```toml
# 各クレートの Cargo.toml
[dev-dependencies]
fraktor-utils-core-rs = { ..., features = ["debug-locks"] }
```

`cargo test` 時は dev-dependencies が resolve されるため、`debug-locks` feature が自動的に有効になる。production build では dev-dependencies は含まれないため `SpinSyncMutex` が使われる。

代替案:
- `cfg(test)` で切替: 依存クレートに伝播しないため不採用
- 手動で `--features debug-locks` を渡す: CI で忘れる可能性があるため不採用

### 5. utils-core 版の debug 型は `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` と命名する

utils-adaptor-std に既存の `DebugSpinSyncMutex`（`std::thread` ベース）と名前が衝突するため、utils-core 版は `CheckedSpinSyncMutex` / `CheckedSpinSyncRwLock` と命名する。「Checked」は Rust の慣例（`checked_add` 等）に倣い、追加検査付きバリアントであることを示す。

utils-adaptor-std の既存 `DebugSpinSyncMutex` はスレッド ID 付き詳細診断が必要な場面で引き続き直接使える。

代替案:
- 同名にして qualified path で区別する: import 時の混乱とコンパイルエラーの原因になるため不採用
- utils-adaptor-std 版を削除する: スレッド ID 付き診断の価値を捨てるため不採用

## Risks / Trade-offs

- [Risk] `debug-locks` feature が dev-dependencies 経由で有効化されると、テスト時の SharedLock のレイアウトが production と異なる → Mitigation: `AtomicBool` 1 つの追加のみで、lock semantics は同一。production で起きない再入バグがテストで panic として検知されるのは意図通り
- [Risk] `AtomicBool` ベースの検知は TOCTOU 窓がある（マルチスレッドで swap と lock 取得の間に別スレッドが介入） → Mitigation: 完全な検知ではなくベストエフォート。spin::Mutex 自体が single-owner なので、実際の re-entry は単一スレッドの再帰呼び出しで発生する
- [Risk] `AtomicU8` ステートマシンは read の参照カウントを持たないため、複数スレッドからの concurrent read を正確に追跡できない → Mitigation: 目的は single-thread の再入検知であり、concurrent read tracking は non-goal。`read()` → `read()` は spin::RwLock がデッドロックしないため問題にならない

## Open Questions

- なし（探索モードで設計判断は確定済み）
