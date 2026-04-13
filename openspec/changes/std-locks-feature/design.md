## Context

`DefaultMutex<T>` / `DefaultRwLock<T>` type alias は utils-core に定義され、feature flag で compile-time にバックエンドが切り替わる。現在 `debug-locks` で `CheckedSpinSyncMutex` への切替が可能。次に `StdSyncMutex` への切替を追加する。

`StdSyncMutex` / `StdSyncRwLock` は `std::sync::Mutex` / `std::sync::RwLock` に依存するが、utils-core は `no_std` クレートである。`cfg(feature = "std-locks")` で条件付きコンパイルし、feature が無効な場合は std に一切依存しない。

## Goals / Non-Goals

**Goals:**
- utils-core に `std-locks` feature と `StdSyncMutex` / `StdSyncRwLock` を新設する
- `DefaultMutex` / `DefaultRwLock` の解決に `std-locks` 分岐を追加する
- actor-adaptor-std が feature を有効化するだけで全クレートに波及する構成にする

**Non-Goals:**
- actor-core に `#[cfg(feature = "std")]` を追加すること
- utils-adaptor-std の既存 `StdSyncMutex` / `StdSyncRwLock` を削除すること
- `debug-locks` と `std-locks` の両立時に runtime 切替を提供すること

## Decisions

### 1. utils-core に `std` と `std-locks` feature を追加する

`std-locks` feature は `std` feature に依存する。`std` feature は `extern crate std;` を有効化し、`std-locks` はその上で `StdSyncMutex` / `StdSyncRwLock` モジュールをコンパイルする。

```toml
[features]
std = []
std-locks = ["std"]
```

`std` feature を分離するのは、将来 std の他の機能（I/O, thread 等）を個別に有効化できるようにするため。

代替案:
- `std-locks` だけで暗黙的に std を引き込む: feature 粒度が粗くなるが、現時点では `std` の他用途がないため許容できる。ただし将来の拡張性を考え分離する

### 2. DefaultMutex の解決優先順位は debug-locks > std-locks > default

```rust
#[cfg(feature = "debug-locks")]
pub type DefaultMutex<T> = CheckedSpinSyncMutex<T>;

#[cfg(all(feature = "std-locks", not(feature = "debug-locks")))]
pub type DefaultMutex<T> = StdSyncMutex<T>;

#[cfg(not(any(feature = "debug-locks", feature = "std-locks")))]
pub type DefaultMutex<T> = SpinSyncMutex<T>;
```

`debug-locks` が最優先。`cargo test` 時に dev-dependencies で `debug-locks` が有効になると、`std-locks` より優先して `CheckedSpinSyncMutex` が使われる。これによりテスト時の再入検知が確実に動作する。

代替案:
- `std-locks` を優先する: テスト時に再入検知が無効になるため不採用
- 両方有効を禁止する: Cargo の feature unification では制御困難なため不採用

### 3. StdSyncMutex / StdSyncRwLock は utils-core に新設し、utils-adaptor-std は re-export にする

utils-core に `StdSyncMutex` / `StdSyncRwLock` を `cfg(feature = "std-locks")` で新設する。utils-adaptor-std の既存実装は削除し、utils-core からの re-export に置き換える。依存方向は utils-adaptor-std → utils-core なので re-export は可能。

```rust
// utils-adaptor-std/src/std/sync/std_sync_mutex.rs
pub use fraktor_utils_core_rs::core::sync::StdSyncMutex;
```

これにより型の二重定義を回避し、実体を utils-core に一本化する。

代替案:
- 両方に実体を残す: 二重定義になりメンテナンス負荷が増えるため不採用
- utils-core 版を utils-adaptor-std から re-export する: 依存方向が逆（core が adaptor を知る）なので不可能

### 4. actor-adaptor-std が utils-core の std-locks を有効化する

```toml
# actor-adaptor-std/Cargo.toml
[dependencies]
fraktor-utils-core-rs = { workspace = true, features = ["std-locks"] }
```

Cargo の feature unification により、同一ビルドグラフ内の全クレート（actor-core 含む）で `DefaultMutex` = `StdSyncMutex` に解決される。actor-core のソースコード・Cargo.toml は変更不要。

## Risks / Trade-offs

- [Risk] `std-locks` と `debug-locks` が同時有効時の挙動が直感的でない → Mitigation: `debug-locks` 優先ルールを明文化。`cargo test` 時は dev-deps の `debug-locks` が勝つので再入検知は常に有効
- [Risk] utils-core と utils-adaptor-std に同名の `StdSyncMutex` が共存する → Mitigation: compile-time-lock-backend-selection で `Checked*` と同じ対処。`DefaultMutex` 経由で使うため直接参照する場面はほぼない
- [Risk] no_std ターゲットで `std-locks` が有効化されるとビルドが壊れる → Mitigation: 組み込みターゲットでは actor-adaptor-std を依存に含めないため feature が有効化されない

## Open Questions

- なし
